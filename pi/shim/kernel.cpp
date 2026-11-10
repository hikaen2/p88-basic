#include "kernel.h"
#include <circle/alloc.h>
#include <circle/util.h>
#include <circle/bcmpropertytags.h>
#include <circle/string.h>

#define PROPTAG_GET_PIXEL_ORDER	0x00040006	// 0: BGR, 1: RGB
#define DRIVE			"SD:"
#define P88_DIR			DRIVE "/P88"	// プログラムの保存先

static const char From[] = "p88";

CKernel *CKernel::s_pThis = 0;

extern "C" void rust_main (void);	// crates/p88-pi。戻らない

CKernel::CKernel (void)
:	m_Timer (&m_Interrupt),
	m_Logger (m_Options.GetLogLevel (), &m_Timer),
	m_USBHCI (&m_Interrupt, &m_Timer, TRUE),	// TRUE: 抜き差し対応
	m_EMMC (&m_Interrupt, &m_Timer, &m_ActLED),
	m_Sound (&m_Interrupt, 48000),
	m_pFrameBuffer (0),
	m_bRGB (FALSE),
	m_bDisk (FALSE),
	m_bSound (FALSE),
	m_pKeyboard (0),
	m_nKeyIn (0),
	m_nKeyOut (0)
{
	s_pThis = this;
}

boolean CKernel::Initialize (void)
{
	if (!m_Serial.Initialize (115200))
		return FALSE;
	if (!m_Logger.Initialize (&m_Serial))	// ログはシリアルへ (画面は BASIC が使う)
		return FALSE;
	if (!m_Interrupt.Initialize ())
		return FALSE;
	if (!m_Timer.Initialize ())
		return FALSE;

	m_pFrameBuffer = new CBcmFrameBuffer (P88_WIDTH, P88_HEIGHT, 32);
	if (!m_pFrameBuffer->Initialize ())
	{
		m_Logger.Write (From, LogError, "Cannot initialize frame buffer");
		return FALSE;
	}

	// 並び順はファームウェア (と QEMU) で異なりうるので問い合わせて合わせる
	CBcmPropertyTags Tags;
	TPropertyTagSimple Order;
	if (Tags.GetTag (PROPTAG_GET_PIXEL_ORDER, &Order, sizeof Order))
		m_bRGB = Order.nValue == 1;
	m_Logger.Write (From, LogNotice, "Pixel order: %s", m_bRGB ? "RGB" : "BGR");

	if (!m_USBHCI.Initialize ())
		return FALSE;

	// SD カード: 使えなくても起動は続ける (SAVE などが Disk I/O error になる)
	if (m_EMMC.Initialize () && f_mount (&m_FileSystem, DRIVE, 1) == FR_OK)
	{
		FRESULT r = f_mkdir (P88_DIR);
		m_bDisk = r == FR_OK || r == FR_EXIST;
	}
	m_Logger.Write (From, LogNotice, "SD card: %s", m_bDisk ? "ok" : "not available");

	// HDMI 音声: 使えなくても起動は続ける (BEEP が鳴らないだけ)
	if (m_Sound.AllocateQueue (200))
	{
		m_Sound.SetWriteFormat (SoundFormatSigned16, 1);
		m_bSound = m_Sound.Start ();
	}
	m_Logger.Write (From, LogNotice, "HDMI sound: %s", m_bSound ? "ok" : "not available");

	return TRUE;
}

void CKernel::Run (void)
{
	m_Logger.Write (From, LogNotice, "Starting rust_main");
	rust_main ();
}

void CKernel::Update (void)
{
	if (m_USBHCI.UpdatePlugAndPlay () && m_pKeyboard == 0)
	{
		m_pKeyboard = (CUSBKeyboardDevice *) m_DeviceNameService.GetDevice ("ukbd1", FALSE);
		if (m_pKeyboard != 0)
		{
			m_pKeyboard->RegisterRemovedHandler (KeyboardRemovedHandler);
			m_pKeyboard->RegisterKeyStatusHandlerRaw (KeyStatusHandler);
			m_Logger.Write (From, LogNotice, "Keyboard attached (%s)", IsKeymapUS () ? "US" : "JIS");
		}
	}
}

boolean CKernel::IsKeymapUS (void) const
{
	const char *pMap = m_Options.GetKeyMap ();
	return pMap != 0 && strcmp (pMap, "US") == 0;
}

// 割り込みで呼ばれる。満杯なら捨てる。
void CKernel::KeyStatusHandler (unsigned char ucModifiers, const unsigned char RawKeys[6])
{
	CKernel *p = s_pThis;
	unsigned nNext = (p->m_nKeyIn + 1) % KeyQueueSize;
	if (nNext == p->m_nKeyOut)
		return;
	u8 *pEntry = p->m_KeyQueue[p->m_nKeyIn];
	pEntry[0] = ucModifiers;
	memcpy (pEntry + 1, RawKeys, 6);
	p->m_nKeyIn = nNext;
}

void CKernel::KeyboardRemovedHandler (CDevice *pDevice, void *pContext)
{
	s_pThis->m_pKeyboard = 0;
}

boolean CKernel::PollKeyReport (u8 *pModifiers, u8 Keys[6])
{
	if (m_nKeyOut == m_nKeyIn)
		return FALSE;
	const u8 *pEntry = m_KeyQueue[m_nKeyOut];
	*pModifiers = pEntry[0];
	memcpy (Keys, pEntry + 1, 6);
	m_nKeyOut = (m_nKeyOut + 1) % KeyQueueSize;
	return TRUE;
}

// ---- Rust へ公開する C ABI ----

// 640x400 の 0x00RRGGBB バッファを画面へ転送する
extern "C" void p88_present (const u32 *pBuffer)
{
	CBcmFrameBuffer *pFB = CKernel::Get ()->FrameBuffer ();
	u8 *pDest = (u8 *) (uintptr) pFB->GetBuffer ();
	u32 nPitch = pFB->GetPitch ();
	boolean bRGB = CKernel::Get ()->IsRGB ();
	for (unsigned y = 0; y < P88_HEIGHT; y++)
	{
		u32 *pLine = (u32 *) (pDest + y * nPitch);
		const u32 *pSrc = pBuffer + y * P88_WIDTH;
		for (unsigned x = 0; x < P88_WIDTH; x++)
		{
			u32 c = pSrc[x];
			if (bRGB)	// 0x00RRGGBB -> メモリ上 R,G,B の順
				c = (c & 0x00FF00) | (c >> 16 & 0xFF) | (c & 0xFF) << 16;
			pLine[x] = c | 0xFF000000;
		}
	}
}

extern "C" u64 p88_millis (void)
{
	return CTimer::GetClockTicks64 () / 1000;	// クロックは 1MHz
}

extern "C" void p88_update (void)
{
	CKernel::Get ()->Update ();
}

extern "C" int p88_kbd_poll (u8 *pModifiers, u8 *pKeys)
{
	return CKernel::Get ()->PollKeyReport (pModifiers, pKeys);
}

extern "C" int p88_keymap_us (void)
{
	return CKernel::Get ()->IsKeymapUS ();
}

extern "C" void *p88_malloc (size_t nSize, size_t nAlign)
{
	return nAlign <= 16 ? malloc (nSize) : memalign (nAlign, nSize);
}

extern "C" void p88_free (void *p)
{
	free (p);
}

extern "C" void p88_log (const char *pMsg, size_t nLen)
{
	char Buf[256];
	if (nLen >= sizeof Buf)
		nLen = sizeof Buf - 1;
	memcpy (Buf, pMsg, nLen);
	Buf[nLen] = '\0';
	CLogger::Get ()->Write (From, LogNotice, "%s", Buf);
}

// ---- ファイル: 戻り値 0 = 成功、1 = 見つからない、2 = 入出力エラー ----

static int FileResult (FRESULT r)
{
	if (r == FR_OK)
		return 0;
	if (r == FR_NO_FILE || r == FR_NO_PATH)
		return 1;
	return 2;
}

static void MakePath (char *pPath, size_t nSize, const char *pName)
{
	CString s;
	s.Format ("%s/%s", P88_DIR, pName);
	strncpy (pPath, s, nSize - 1);
	pPath[nSize - 1] = '\0';
}

// 読んだ内容は p88_malloc で確保して返す (呼び出し側が p88_free する)
extern "C" int p88_file_read (const char *pName, u8 **ppData, size_t *pLen)
{
	if (!CKernel::Get ()->HasDisk ())
		return 2;
	char Path[300];
	MakePath (Path, sizeof Path, pName);
	FIL File;
	FRESULT r = f_open (&File, Path, FA_READ | FA_OPEN_EXISTING);
	if (r != FR_OK)
		return FileResult (r);
	size_t nLen = f_size (&File);
	u8 *pData = (u8 *) malloc (nLen + 1);
	UINT nRead = 0;
	r = f_read (&File, pData, nLen, &nRead);
	f_close (&File);
	if (r != FR_OK || nRead != nLen)
	{
		free (pData);
		return 2;
	}
	*ppData = pData;
	*pLen = nLen;
	return 0;
}

extern "C" int p88_file_write (const char *pName, const u8 *pData, size_t nLen)
{
	if (!CKernel::Get ()->HasDisk ())
		return 2;
	char Path[300];
	MakePath (Path, sizeof Path, pName);
	FIL File;
	FRESULT r = f_open (&File, Path, FA_WRITE | FA_CREATE_ALWAYS);
	if (r != FR_OK)
		return FileResult (r);
	UINT nWritten = 0;
	r = f_write (&File, pData, nLen, &nWritten);
	FRESULT r2 = f_close (&File);
	return r == FR_OK && r2 == FR_OK && nWritten == nLen ? 0 : 2;
}

// ファイルごとに pCallback (pContext, 名前, サイズ) を呼ぶ
extern "C" int p88_file_list (void (*pCallback) (void *, const char *, u64), void *pContext)
{
	if (!CKernel::Get ()->HasDisk ())
		return 2;
	DIR Dir;
	FILINFO Info;
	FRESULT r = f_findfirst (&Dir, &Info, P88_DIR, "*");
	while (r == FR_OK && Info.fname[0] != '\0')
	{
		if (!(Info.fattrib & (AM_DIR | AM_HID | AM_SYS)))
			(*pCallback) (pContext, Info.fname, Info.fsize);
		r = f_findnext (&Dir, &Info);
	}
	f_closedir (&Dir);
	return FileResult (r);
}

extern "C" int p88_file_delete (const char *pName)
{
	if (!CKernel::Get ()->HasDisk ())
		return 2;
	char Path[300];
	MakePath (Path, sizeof Path, pName);
	return FileResult (f_unlink (Path));
}

// ---- 音 ----

// 2kHz の矩形波を約 0.1 秒鳴らす
extern "C" void p88_beep (void)
{
	CHDMISoundBaseDevice *pSound = CKernel::Get ()->Sound ();
	if (pSound == 0)
		return;
	static s16 Wave[4800];	// 48kHz x 0.1 秒、モノラル
	for (unsigned i = 0; i < sizeof Wave / sizeof Wave[0]; i++)
		Wave[i] = (i / 12) % 2 ? 6000 : -6000;	// 12 サンプルごとに反転 = 2kHz
	pSound->Write (Wave, sizeof Wave);
}

// libgcc の unwind 補助から参照される
extern "C" void abort (void)
{
	CLogger::Get ()->Write (From, LogPanic, "abort");
	for (;;);
}
