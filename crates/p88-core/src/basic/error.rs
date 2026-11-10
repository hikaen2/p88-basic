//! BASIC のエラー (N88-BASIC 風の英語メッセージ)

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Error {
    Syntax,
    UndefinedLine,
    DivisionByZero,
    TypeMismatch,
    OutOfMemory,
    Overflow,
    IllegalFunctionCall,
    ReturnWithoutGosub,
    NextWithoutFor,
    OutOfData,
    SubscriptOutOfRange,
    DuplicateDefinition,
    CantContinue,
    FileNotFound,
    DiskIo,
    DirectStatementInFile,
    BadFileName,
}

impl Error {
    pub fn message(self) -> &'static str {
        match self {
            Error::Syntax => "Syntax error",
            Error::UndefinedLine => "Undefined line number",
            Error::DivisionByZero => "Division by zero",
            Error::TypeMismatch => "Type mismatch",
            Error::OutOfMemory => "Out of memory",
            Error::Overflow => "Overflow",
            Error::IllegalFunctionCall => "Illegal function call",
            Error::ReturnWithoutGosub => "RETURN without GOSUB",
            Error::NextWithoutFor => "NEXT without FOR",
            Error::OutOfData => "Out of DATA",
            Error::SubscriptOutOfRange => "Subscript out of range",
            Error::DuplicateDefinition => "Duplicate Definition",
            Error::CantContinue => "Can't continue",
            Error::FileNotFound => "File not found",
            Error::DiskIo => "Disk I/O error",
            Error::DirectStatementInFile => "Direct statement in file",
            Error::BadFileName => "Bad file name",
        }
    }
}

pub type Result<T> = core::result::Result<T, Error>;
