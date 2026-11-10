// Circle の初期化だけを行う薄い層。機能は extern "C" の p88_* 関数で Rust に公開する。
#ifndef _kernel_h
#define _kernel_h

#include <circle/actled.h>
#include <circle/koptions.h>
#include <circle/devicenameservice.h>
#include <circle/serial.h>
#include <circle/exceptionhandler.h>
#include <circle/interrupt.h>
#include <circle/timer.h>
#include <circle/logger.h>
#include <circle/bcmframebuffer.h>
#include <circle/usb/usbhcidevice.h>
#include <circle/usb/usbkeyboard.h>
#include <circle/sound/hdmisoundbasedevice.h>
#include <SDCard/emmc.h>
#include <fatfs/ff.h>
#include <circle/types.h>

#define P88_WIDTH	640
#define P88_HEIGHT	400

class CKernel
{
public:
	CKernel (void);

	boolean Initialize (void);
	void Run (void);

	static CKernel *Get (void) { return s_pThis; }

	CBcmFrameBuffer *FrameBuffer (void) { return m_pFrameBuffer; }
	boolean IsRGB (void) const { return m_bRGB; }
	boolean HasDisk (void) const { return m_bDisk; }
	CHDMISoundBaseDevice *Sound (void) { return m_bSound ? &m_Sound : 0; }

	// メインループから呼ぶ: USB 機器の接続・切断を処理する
	void Update (void);

	// 配列の設定 (cmdline.txt の keymap=US で US、それ以外は JIS)
	boolean IsKeymapUS (void) const;

	// キーボードの報告 (修飾キー + キーコード 6 個) を 1 つ取り出す
	boolean PollKeyReport (u8 *pModifiers, u8 Keys[6]);

private:
	// 初期化順を変えないこと
	CActLED			m_ActLED;
	CKernelOptions		m_Options;
	CDeviceNameService	m_DeviceNameService;
	CSerialDevice		m_Serial;
	CExceptionHandler	m_ExceptionHandler;
	CInterruptSystem	m_Interrupt;
	CTimer			m_Timer;
	CLogger			m_Logger;
	CUSBHCIDevice		m_USBHCI;
	CEMMCDevice		m_EMMC;
	FATFS			m_FileSystem;
	CHDMISoundBaseDevice	m_Sound;
	CBcmFrameBuffer		*m_pFrameBuffer;
	boolean			m_bRGB;		// 画素の並び順 (TRUE: 下位バイトが R)
	boolean			m_bDisk;	// SD カードが使える
	boolean			m_bSound;	// HDMI 音声が使える
	CUSBKeyboardDevice * volatile m_pKeyboard;

	// キーボードの報告のリングバッファ (割り込みで積み、メインループで取り出す)
	static const unsigned KeyQueueSize = 32;
	u8			m_KeyQueue[KeyQueueSize][7];
	volatile unsigned	m_nKeyIn;
	volatile unsigned	m_nKeyOut;

	static void KeyStatusHandler (unsigned char ucModifiers, const unsigned char RawKeys[6]);
	static void KeyboardRemovedHandler (CDevice *pDevice, void *pContext);

	static CKernel *s_pThis;
};

#endif
