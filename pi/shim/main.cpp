// p88 カーネルの入口。CKernel を初期化して Rust 側へ制御を渡す。
#include "kernel.h"
#include <circle/startup.h>

int main (void)
{
	CKernel Kernel;
	if (!Kernel.Initialize ())
	{
		halt ();
		return EXIT_HALT;
	}

	Kernel.Run ();	// 戻らない
	halt ();
	return EXIT_HALT;
}
