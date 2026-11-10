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
	CBcmFrameBuffer		*m_pFrameBuffer;
	boolean			m_bRGB;		// 画素の並び順 (TRUE: 下位バイトが R)

	static CKernel *s_pThis;
};

#endif
