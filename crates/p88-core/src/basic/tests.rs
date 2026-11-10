//! BASIC の振る舞いのテスト (spec の basic-language のシナリオ)。
//! 行をキー入力で打ち込み、実行後の画面の文字を調べる。

use crate::key::Key;
use crate::machine::tests::{Fake, row};
use crate::machine::{Machine, Mode};
use alloc::string::String;
use alloc::vec::Vec;

pub fn machine() -> Machine<Fake> {
    let mut m = Machine::new(Fake::default());
    m.text.clear();
    m
}

/// 行を画面に書いて RET を押し、実行が止まる (編集か入力待ち) まで進める。
/// (キーボードからは ASCII しか打てないので、日本語を含む行も扱えるよう画面へ直接書く)
pub fn type_line(m: &mut Machine<Fake>, line: &str) {
    m.text.put_str(line);
    m.platform.keys.push_back(Key::Enter);
    settle(m);
}

pub fn settle(m: &mut Machine<Fake>) {
    for _ in 0..10_000 {
        m.step();
        if !matches!(m.mode, Mode::Run) && m.platform.keys.is_empty() {
            return;
        }
    }
    panic!("did not stop");
}

/// 画面の空でない行
pub fn lines(m: &Machine<Fake>) -> Vec<String> {
    (0..m.text.rows()).map(|y| row(&m.text, y)).filter(|s| !s.is_empty()).collect()
}

/// 行を順に入力し、画面の空でない行を返す (画面は最初に消す)
pub fn run(src: &[&str]) -> Vec<String> {
    let mut m = machine();
    for l in src {
        type_line(&mut m, l);
    }
    lines(&m)
}

/// 最後の行を実行した結果の出力 (最後に入力した行より後ろ、末尾の Ok を除く)
pub fn out(src: &[&str]) -> Vec<String> {
    let all = run(src);
    let last = src.last().unwrap();
    let i = all.iter().rposition(|l| l == last).map_or(0, |i| i + 1);
    let mut v: Vec<String> = all[i..].to_vec();
    if v.last().is_some_and(|l| l == "Ok") {
        v.pop();
    }
    v
}

#[test]
fn add_and_replace_lines() {
    assert_eq!(out(&["20 PRINT 2", "10 PRINT 1", "20 PRINT 3", "LIST"]), ["10 PRINT 1", "20 PRINT 3"]);
}

#[test]
fn delete_line() {
    assert_eq!(out(&["10 PRINT 1", "10", "LIST"]), Vec::<String>::new());
}

#[test]
fn direct_mode() {
    assert_eq!(run(&["PRINT 1+2"]), ["PRINT 1+2", " 3", "Ok"]);
}

#[test]
fn new_clears_program() {
    assert_eq!(out(&["10 PRINT 1", "NEW", "LIST"]), Vec::<String>::new());
}

#[test]
fn list_range() {
    assert_eq!(out(&["10 A=1", "20 B=2", "30 C=3", "LIST 20-30"]), ["20 B=2", "30 C=3"]);
    assert_eq!(out(&["10 A=1", "20 B=2", "30 C=3", "LIST -20"]), ["10 A=1", "20 B=2"]);
    assert_eq!(out(&["10 A=1", "20 B=2", "LIST 20"]), ["20 B=2"]);
}

#[test]
fn list_uppercases() {
    assert_eq!(out(&["10 print \"abc\";a", "LIST"]), ["10 PRINT \"abc\";A"]);
}

// ---- 5.2 値・変数・配列・式 ----

#[test]
fn integer_assignment_rounds() {
    assert_eq!(out(&["A%=7.6:PRINT A%"]), [" 8"]);
}

#[test]
fn arrays() {
    assert_eq!(out(&["DIM B(20):B(15)=3:PRINT B(15)"]), [" 3"]);
    assert_eq!(out(&["C(10)=4:PRINT C(10)"]), [" 4"]);
    assert_eq!(out(&["D(11)=1"]), ["Subscript out of range"]);
    assert_eq!(out(&["DIM E$(2,3):E$(2,3)=\"x\":PRINT E$(2,3)"]), ["x"]);
    assert_eq!(out(&["DIM F(1):DIM F(2)"]), ["Duplicate Definition"]);
}

#[test]
fn precedence() {
    assert_eq!(out(&["PRINT 2+3*4^2"]), [" 50"]);
    assert_eq!(out(&["PRINT -2^2"]), ["-4"]);
    assert_eq!(out(&["PRINT 7\\2;7 MOD 3;7/2"]), [" 3  1  3.5"]);
    assert_eq!(out(&["PRINT 1+2=3 AND 4>5"]), [" 0"]);
}

#[test]
fn comparison_and_logic() {
    assert_eq!(out(&["PRINT (1<2)"]), ["-1"]);
    assert_eq!(out(&["PRINT NOT 0;5 OR 2;6 XOR 3"]), ["-1  7  5"]);
    assert_eq!(out(&["PRINT \"ABC\"<\"ABD\""]), ["-1"]);
}

#[test]
fn hex_constants() {
    assert_eq!(out(&["PRINT &H10;&HFFFF"]), [" 16 -1"]);
}

#[test]
fn integer_overflow_promotes() {
    assert_eq!(out(&["PRINT 200*200"]), [" 40000"]);
}

#[test]
fn variable_types_are_separate() {
    assert_eq!(out(&["A=1.5:A%=2:A$=\"s\":PRINT A;A%;A$"]), [" 1.5  2 s"]);
}

#[test]
fn type_mismatch() {
    assert_eq!(out(&["A$=1"]), ["Type mismatch"]);
    assert_eq!(out(&["PRINT \"A\"+1"]), ["Type mismatch"]);
}

// ---- 5.3 PRINT・ダイレクトモード・エラー ----

#[test]
fn print_separators() {
    assert_eq!(out(&["PRINT \"A\";\"B\",\"C\""]), ["AB            C"]);
    assert_eq!(out(&["PRINT 1;:PRINT 2"]), [" 1  2"]);
    assert_eq!(out(&["PRINT"]), Vec::<String>::new());
}

#[test]
fn syntax_error_in_program() {
    assert_eq!(out(&["10 PRINT (", "RUN"]), ["Syntax error in 10"]);
}

#[test]
fn division_by_zero() {
    assert_eq!(out(&["PRINT 1/0"]), ["Division by zero"]);
}

#[test]
fn undefined_line() {
    assert_eq!(out(&["10 GOTO 99", "RUN"]), ["Undefined line number in 10"]);
}

// ---- 5.4 制御構文 ----

#[test]
fn for_loop() {
    assert_eq!(out(&["10 FOR I=1 TO 3:PRINT I;:NEXT", "RUN"]), [" 1  2  3"]);
    assert_eq!(out(&["FORI=10TO1STEP-3:PRINTI;:NEXTI"]), [" 10  7  4  1"]);
}

#[test]
fn for_loop_not_entered() {
    assert_eq!(out(&["10 FOR I=5 TO 1", "20 PRINT \"X\"", "30 NEXT", "40 PRINT \"E\"", "RUN"]), ["E"]);
}

#[test]
fn nested_for_with_combined_next() {
    assert_eq!(out(&["FOR I=1 TO 2:FOR J=1 TO 2:PRINT I*10+J;:NEXT J,I"]), [" 11  12  21  22"]);
}

#[test]
fn if_else() {
    assert_eq!(out(&["10 A=5:IF A>3 THEN PRINT \"Y\" ELSE PRINT \"N\"", "RUN"]), ["Y"]);
    assert_eq!(out(&["A=1:IF A>3 THEN PRINT \"Y\" ELSE PRINT \"N\""]), ["N"]);
    assert_eq!(out(&["IF 0 THEN PRINT 1:PRINT 2"]), Vec::<String>::new());
    assert_eq!(out(&["IF 1 THEN PRINT 1:PRINT 2"]), [" 1", " 2"]);
}

#[test]
fn if_then_line_number() {
    assert_eq!(out(&["10 IF 1 THEN 30", "20 PRINT \"NO\"", "30 PRINT \"YES\"", "RUN"]), ["YES"]);
    assert_eq!(out(&["10 IF 0 THEN 30 ELSE 40", "30 PRINT \"T\":END", "40 PRINT \"F\"", "RUN"]), ["F"]);
}

#[test]
fn gosub_return() {
    assert_eq!(out(&["10 GOSUB 100:END", "100 PRINT \"S\":RETURN", "RUN"]), ["S"]);
    assert_eq!(out(&["RETURN"]), ["RETURN without GOSUB"]);
}

#[test]
fn rem_and_apostrophe() {
    assert_eq!(out(&["10 REM x:PRINT 1", "20 PRINT 2 ' note", "RUN"]), [" 2"]);
}

#[test]
fn stop_and_cont() {
    assert_eq!(out(&["10 PRINT 1:STOP:PRINT 2", "RUN"]), [" 1", "Break in 10"]);
    assert_eq!(out(&["10 PRINT 1:STOP:PRINT 2", "RUN", "CONT"]), [" 2"]);
    assert_eq!(out(&["CONT"]), ["Can't continue"]);
}

#[test]
fn next_without_for() {
    assert_eq!(out(&["NEXT"]), ["NEXT without FOR"]);
}

// ---- 5.5 関数 ----

#[test]
fn string_functions_by_code_point() {
    assert_eq!(out(&["PRINT LEN(\"あいう\")"]), [" 3"]);
    assert_eq!(out(&["PRINT CHR$(&H3042);ASC(\"あ\")"]), ["あ 12354"]);
    assert_eq!(out(&["PRINT MID$(\"あいう\",2,1);LEFT$(\"ABC\",2);RIGHT$(\"ABC\",1)"]), ["いABC"]);
    assert_eq!(out(&["PRINT INSTR(\"ABCABC\",\"CA\");INSTR(4,\"ABCABC\",\"B\")"]), [" 3  5"]);
    assert_eq!(out(&["PRINT STR$(5)+\"!\";VAL(\" 12.5X\")"]), [" 5! 12.5"]);
    assert_eq!(out(&["PRINT STRING$(3,\"*\");SPACE$(2);\"|\";HEX$(255)"]), ["***  |FF"]);
}

#[test]
fn numeric_functions() {
    assert_eq!(out(&["PRINT INT(-2.5)"]), ["-3"]);
    assert_eq!(out(&["PRINT FIX(-2.5);ABS(-3);SGN(-7);SQR(16)"]), ["-2  3 -1  4"]);
    assert_eq!(out(&["PRINT INT(SIN(0)*100);INT(COS(0));INT(EXP(1)*1000);INT(LOG(100)*1000)"]), [" 0  1  2718  4605"]);
    assert_eq!(out(&["PRINT SQR(-1)"]), ["Illegal function call"]);
}

#[test]
fn rnd_in_range() {
    assert_eq!(out(&["FOR I=1 TO 100:R=RND(1):IF R<0 OR R>=1 THEN PRINT R:NEXT"]).len(), 0);
}

// ---- 5.6 INPUT・READ/DATA ----

#[test]
fn input_number() {
    let mut m = machine();
    type_line(&mut m, "10 INPUT \"N\";N:PRINT N*2");
    type_line(&mut m, "RUN");
    assert!(matches!(m.mode, Mode::Input(_)));
    assert_eq!(lines(&m).last().unwrap(), "N?");
    type_line(&mut m, "21");
    assert_eq!(lines(&m)[2..], ["N? 21", " 42", "Ok"]);
}

#[test]
fn input_redo() {
    let mut m = machine();
    type_line(&mut m, "INPUT A,B$:PRINT A;B$");
    type_line(&mut m, "X");
    type_line(&mut m, "3,\"a,b\"");
    assert_eq!(lines(&m)[1..], ["? X", "?Redo from start", "? 3,\"a,b\"", " 3 a,b", "Ok"]);
}

#[test]
fn line_input() {
    let mut m = machine();
    type_line(&mut m, "LINE INPUT \"S:\";S$:PRINT \"[\";S$;\"]\"");
    type_line(&mut m, "a, b");
    assert_eq!(lines(&m)[1..], ["S:a, b", "[a, b]", "Ok"]);
}

#[test]
fn read_data() {
    assert_eq!(out(&["10 READ A,B$:PRINT A;B$", "20 DATA 5,XY", "RUN"]), [" 5 XY"]);
    assert_eq!(out(&["10 READ A:PRINT A;:GOTO 10", "20 DATA 1,2", "RUN"]), [" 1  2", "Out of DATA in 10"]);
    assert_eq!(out(&["10 READ A:RESTORE:READ B:PRINT A;B", "20 DATA 7,8", "RUN"]), [" 7  7"]);
}

#[test]
fn inkey_empty() {
    assert_eq!(out(&["PRINT LEN(INKEY$)"]), [" 0"]);
}

// ---- 5.7 中断と再開 ----

/// 行を入力して RET し、何回か step する (止まらないプログラム用)
fn start(m: &mut Machine<Fake>, line: &str) {
    m.text.put_str(line);
    m.platform.keys.push_back(Key::Enter);
    for _ in 0..5 {
        m.step();
    }
}

#[test]
fn break_and_cont() {
    let mut m = machine();
    type_line(&mut m, "10 A=A+1:GOTO 10");
    start(&mut m, "RUN");
    assert!(matches!(m.mode, Mode::Run));
    m.platform.keys.push_back(Key::Stop);
    m.step();
    assert_eq!(lines(&m)[2..], ["Break in 10", "Ok"]);
    type_line(&mut m, "PRINT A>0");
    assert_eq!(lines(&m)[4..], ["PRINT A>0", "-1", "Ok"]);
    start(&mut m, "CONT");
    assert!(matches!(m.mode, Mode::Run));
    m.platform.keys.push_back(Key::Stop);
    m.step();
    assert_eq!(lines(&m).last().unwrap(), "Ok");
    assert_eq!(lines(&m)[lines(&m).len() - 2], "Break in 10");
}

#[test]
fn break_during_input_and_cont_reasks() {
    let mut m = machine();
    type_line(&mut m, "10 INPUT X:PRINT X");
    type_line(&mut m, "RUN");
    m.platform.keys.push_back(Key::Stop);
    settle(&mut m);
    assert_eq!(lines(&m)[2..], ["?", "Break in 10", "Ok"]);
    type_line(&mut m, "CONT");
    assert!(matches!(m.mode, Mode::Input(_)));
}

#[test]
fn keys_while_running_go_to_inkey() {
    let mut m = machine();
    type_line(&mut m, "10 K$=INKEY$:IF K$=\"\" THEN 10");
    type_line(&mut m, "20 PRINT ASC(K$)");
    start(&mut m, "RUN");
    m.platform.keys.push_back(Key::Up);
    settle(&mut m);
    assert_eq!(lines(&m)[3..], [" 30", "Ok"]);
}

// ---- 6.1 画面制御 (text-screen) ----

#[test]
fn eighty_column_print() {
    let all = run(&["PRINT STRING$(80,\"*\")"]);
    assert_eq!(all[1], "*".repeat(80));
    assert_eq!(all[2], "Ok");
}

#[test]
fn wide_and_pos() {
    assert_eq!(out(&["PRINT \"Aあ\";:PRINT POS(0)"]), ["Aあ 3"]);
}

#[test]
fn cls_clears_text() {
    let mut m = machine();
    type_line(&mut m, "PRINT \"HELLO\"");
    type_line(&mut m, "CLS");
    assert_eq!(lines(&m), ["Ok"]);
    assert_eq!(m.text.y, 1);
}

#[test]
fn locate_and_csrlin() {
    let mut m = machine();
    type_line(&mut m, "CLS:LOCATE 10,5:PRINT \"X\";CSRLIN");
    assert_eq!(row(&m.text, 5), "          X 5");
    assert_eq!(out(&["LOCATE 80,0"]), ["Illegal function call"]);
}

#[test]
fn color_text() {
    let mut m = machine();
    type_line(&mut m, "COLOR 2:PRINT \"R\":COLOR 7");
    assert_eq!(m.text.cell(0, 1).color, 2);
    assert_eq!(m.text.color, 7);
    assert_eq!(out(&["COLOR 16"]), ["Illegal function call"]);
}

#[test]
fn width_20() {
    let mut m = machine();
    type_line(&mut m, "WIDTH 80,20");
    assert_eq!(m.text.rows(), 20);
    assert_eq!(out(&["WIDTH 40,25"]), ["Illegal function call"]);
}

#[test]
fn scroll_on_overflow() {
    let all = run(&["FOR I=1 TO 30:PRINT I:NEXT"]);
    assert_eq!(all[all.len() - 2..], [" 30", "Ok"]);
    assert!(!all.contains(&String::from(" 1")));
}

// ---- 6.2 パレット ----

#[test]
fn default_palette_red() {
    let mut m = machine();
    type_line(&mut m, "PSET (0,0),2");
    let mut buf = alloc::vec![0u32; 640 * 400];
    m.render(&mut buf);
    assert_eq!(buf[0], 0xFF0000);
}

#[test]
fn palette_change_recolors_drawn_pixels() {
    let mut m = machine();
    type_line(&mut m, "LINE (0,0)-(99,99),9,BF:COLOR=(9,&HF80)");
    let mut buf = alloc::vec![0u32; 640 * 400];
    m.render(&mut buf);
    assert_eq!(buf[50 * 640 + 50], 0xFF8800);
    type_line(&mut m, "COLOR=(7,&H000):COLOR=NEW");
    assert_eq!(m.palette.0[7], 0xFFF);
    assert_eq!(m.palette.0[9], 0x007);
    assert_eq!(out(&["COLOR=(16,0)"]), ["Illegal function call"]);
}

// ---- 6.3 / 6.4 グラフィック命令 ----

#[test]
fn pset_preset_point() {
    assert_eq!(out(&["PSET (5,5),3:PRINT POINT(5,5);:PRESET (5,5):PRINT POINT(5,5);POINT(-1,0)"]), [" 3  0 -1"]);
    assert_eq!(out(&["PSET (100,50),2:PRINT POINT(100,50)"]), [" 2"]);
}

#[test]
fn line_box_fill_and_clip() {
    let mut m = machine();
    type_line(&mut m, "LINE (10,10)-(20,20),6,BF");
    assert_eq!(m.gfx.point(10, 10), 6);
    assert_eq!(m.gfx.point(20, 20), 6);
    assert_eq!(m.gfx.point(21, 21), 0);
    type_line(&mut m, "LINE (30,30)-(40,40),5,B");
    assert_eq!(m.gfx.point(30, 35), 5);
    assert_eq!(m.gfx.point(35, 35), 0);
    type_line(&mut m, "LINE -(50,40)");
    assert_eq!(m.gfx.point(45, 40), 7);
    assert_eq!(out(&["LINE (-100,-100)-(700,300),7"]), Vec::<String>::new());
    assert_eq!(out(&["LINE (0,0)-(1,1),99"]), ["Illegal function call"]);
}

#[test]
fn line_full_screen_under_text() {
    let mut m = machine();
    type_line(&mut m, "LINE (0,0)-(639,399),4,BF:PRINT \"HELLO\"");
    assert_eq!(m.gfx.point(639, 399), 4);
    assert_eq!(row(&m.text, 1), "HELLO");
}

#[test]
fn circle_and_paint() {
    let mut m = machine();
    type_line(&mut m, "CIRCLE (320,200),50,2:PAINT (320,200),2");
    assert_eq!(m.gfx.point(320, 200), 2);
    assert_eq!(m.gfx.point(320 + 60, 200), 0);
    type_line(&mut m, "CIRCLE (100,100),20,5:PAINT (100,100),3,5");
    assert_eq!(m.gfx.point(100, 100), 3);
    assert_eq!(m.gfx.point(120, 100), 5);
}

#[test]
fn circle_is_round() {
    let mut m = machine();
    type_line(&mut m, "CIRCLE (320,200),80,5");
    assert_eq!(m.gfx.point(400, 200), 5);
    assert_eq!(m.gfx.point(320, 120), 5);
}

#[test]
fn cls_graphics() {
    let mut m = machine();
    type_line(&mut m, "PSET (1,1),3:PRINT \"T\"");
    type_line(&mut m, "CLS 2");
    assert_eq!(m.gfx.point(1, 1), 0);
    assert!(lines(&m).contains(&String::from("T")));
    type_line(&mut m, "PSET (1,1),3:CLS 3");
    assert_eq!(m.gfx.point(1, 1), 0);
    assert_eq!(lines(&m), ["Ok"]);
}

// ---- 6.5 BEEP ----

#[test]
fn beep() {
    let mut m = machine();
    type_line(&mut m, "BEEP");
    assert_eq!(m.platform.beeps, 1);
    assert_eq!(lines(&m), ["BEEP", "Ok"]);
}

// ---- 7.1 プログラムの保存 (program-storage) ----

#[test]
fn save_writes_utf8_list() {
    let mut m = machine();
    type_line(&mut m, "10 PRINT \"あ\"");
    type_line(&mut m, "SAVE \"test\"");
    assert_eq!(m.platform.files["TEST.BAS"], "10 PRINT \"あ\"\n".as_bytes());
    type_line(&mut m, "SAVE \"x.txt\"");
    assert!(m.platform.files.contains_key("X.TXT"));
}

#[test]
fn load_crlf_bom_and_run() {
    let mut m = machine();
    m.platform.files.insert("HELLO.BAS".into(), "\u{FEFF}10 PRINT \"こんにちは\"\r\n\r\n20 END\r\n".as_bytes().into());
    type_line(&mut m, "LOAD \"HELLO\"");
    type_line(&mut m, "RUN");
    assert_eq!(lines(&m)[3..], ["こんにちは", "Ok"]);
    type_line(&mut m, "LIST");
    assert!(lines(&m).contains(&String::from("20 END")));
}

#[test]
fn load_and_run_with_r() {
    let mut m = machine();
    m.platform.files.insert("A.BAS".into(), b"10 PRINT 42\n".to_vec());
    type_line(&mut m, "LOAD \"A\",R");
    assert_eq!(lines(&m)[1..], [" 42", "Ok"]);
}

#[test]
fn load_rejects_direct_statement() {
    let mut m = machine();
    m.platform.files.insert("D.BAS".into(), b"10 PRINT 1\nPRINT 2\n".to_vec());
    type_line(&mut m, "LOAD \"D\"");
    assert_eq!(lines(&m)[1..], ["Direct statement in file", "Ok"]);
}

#[test]
fn load_missing_keeps_program() {
    let mut m = machine();
    type_line(&mut m, "10 PRINT 1");
    type_line(&mut m, "LOAD \"NOFILE\"");
    assert_eq!(lines(&m)[2..], ["File not found", "Ok"]);
    assert!(m.b.program.contains_key(&10));
}

#[test]
fn files_and_kill() {
    let mut m = machine();
    type_line(&mut m, "10 END");
    type_line(&mut m, "SAVE \"A\":SAVE \"B\"");
    type_line(&mut m, "FILES");
    let l = lines(&m);
    assert!(l.iter().any(|s| s.starts_with("A.BAS")));
    assert!(l.iter().any(|s| s.starts_with("B.BAS")));
    type_line(&mut m, "KILL \"A.BAS\"");
    assert!(!m.platform.files.contains_key("A.BAS"));
    assert_eq!(out(&["KILL \"A\""]), ["Bad file name"]);
    assert_eq!(out(&["KILL \"NONE.BAS\""]), ["File not found"]);
}

#[test]
fn no_storage_gives_disk_io_error() {
    struct Bare;
    impl crate::machine::Platform for Bare {
        fn poll_key(&mut self) -> Option<Key> {
            None
        }
        fn millis(&self) -> u64 {
            0
        }
    }
    let mut m = Machine::new(Bare);
    m.enter_line("FILES".into());
    m.step();
    assert_eq!(row(&m.text, 3), "Disk I/O error");
}
