#include "kernel.h"
#include <circle/alloc.h>
#include <circle/util.h>
#include <circle/bcmpropertytags.h>

#define PROPTAG_GET_PIXEL_ORDER	0x00040006	// 0: BGR, 1: RGB

static const char From[] = "p88";

CKernel *CKernel::s_pThis = 0;

extern "C" void rust_main (void);	// crates/p88-pi。戻らない

CKernel::CKernel (void)
:	m_Timer (&m_Interrupt),
	m_Logger (m_Options.GetLogLevel (), &m_Timer),
	m_pFrameBuffer (0),
	m_bRGB (FALSE)
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

	return TRUE;
}

void CKernel::Run (void)
{
	m_Logger.Write (From, LogNotice, "Starting rust_main");
	rust_main ();
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

// libgcc の unwind 補助から参照される
extern "C" void abort (void)
{
	CLogger::Get ()->Write (From, LogPanic, "abort");
	for (;;);
}
