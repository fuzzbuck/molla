use std::fmt::Debug;
use std::process::exit;

/// todo integrations with logging libs

/// info log
pub(crate) fn i<T>(data: T)
where
    T: Into<String>,
{
    println!("[molla] {}", data.into());
}

/// warning log
pub(crate) fn w<T>(data: T)
where
    T: Into<String>,
{
    println!("[WARNING] {}", data.into());
}

/// error log
pub(crate) fn e<T, D>(at: T, cause: D, code: i32)
where
    T: Into<String>,
    D: Debug,
{
    eprintln!("[{:?}]: {:?}", at.into(), cause);
    exit(code);
}
