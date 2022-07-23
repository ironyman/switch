// https://github.com/estk/log4rs/blob/master/examples/log_to_file.rs
use log::{LevelFilter, SetLoggerError};
// use log::{debug, error, info, trace, warn, LevelFilter, SetLoggerError};
use log4rs::{
    append::{
        console::{ConsoleAppender, Target},
        file::FileAppender,
    },
    config::{Appender, Config, Root},
    encode::pattern::PatternEncoder,
    // filter::threshold::ThresholdFilter,
};

pub static mut CURRENT_LOG_LEVEL: log::Level = log::Level::Trace;
pub static mut CURRENT_LOG_GROUPS: Option<std::collections::HashSet<String>> = None;

pub fn initialize_log<IntoString>(level: log::Level, groups: &[&str], file_path: IntoString) -> Result<log4rs::Handle, SetLoggerError>
where IntoString: Into<String> {
    unsafe {
        CURRENT_LOG_LEVEL = level;

        // CURRENT_LOG_GROUPS = Some(std::collections::HashSet::<String>::new());
        // let groups: std::vec::Vec<String> = groups.iter().map(|x| x.to_string()).collect();
        // &groups[..] this is an &[String]
        // CURRENT_LOG_GROUPS = Some(std::collections::HashSet::<String>::from(
        //     ["A"]
        // ));

        // let this = file_path.into() as String;

        // let another: std::vec::Vec<String> = groups.iter().map(|x| {
        //     let x: String = (*x).into();
        //     x
        // }).collect();

        // this is so weird, normally it would dereference x: &IntoString automatically but it wouldn't do it 
        // because we didn't require IntoString: Copy. So requiring IntoString: Copy solved it.
        // Above are my attempts to make this work.
        // Ok that still doesn't work because String does not implement Copy so file_path will no longer work with String.
        // CURRENT_LOG_GROUPS = Some(groups.iter().map(|x| (*x).into() as String).collect());
        let groups: std::vec::Vec<String> = groups.iter().map(|x| x.to_string()).collect();

        CURRENT_LOG_GROUPS = Some(groups.into_iter().collect());
    }

    // let level = log::LevelFilter::Info;

    // Build a stderr logger.
    // let stderr = ConsoleAppender::builder().target(Target::Stderr).build();

    // Logging to log file.
    let logfile = FileAppender::builder()
        // Pattern: https://docs.rs/log4rs/*/log4rs/encode/pattern/index.html
        .encoder(Box::new(PatternEncoder::new("{d(%Y-%m-%d %H:%M:%S)} {l} - {m}\n")))
        .build(std::path::Path::new(&file_path.into()))
        .unwrap();

    // Log Trace level output to file where trace is the default level
    // and the programmatically specified level to stderr.
    let config = Config::builder()
        .appender(Appender::builder().build("logfile", Box::new(logfile)))
        // .appender(
        //     Appender::builder()
        //         .filter(Box::new(ThresholdFilter::new(level)))
        //         .build("stderr", Box::new(stderr)),
        // )
        .build(
            Root::builder()
                .appender("logfile")
                // .appender("stderr")
                .build(LevelFilter::Trace),
        )
        .unwrap();

    // Use this to change log levels at runtime.
    // This means you can change the default log level to trace
    // if you are trying to debug an issue and need more logs on then turn it off
    // once you are done.
    let _handle = log4rs::init_config(config)?;

    // error!("Goes to stderr and file");
    // warn!("Goes to stderr and file");
    // info!("Goes to stderr and file");
    // debug!("Goes to file only");
    // trace!("Goes to file only");

    Ok(_handle)
}


pub fn initialize_test_log(level: log::Level, groups: &[&str]) -> Result<log4rs::Handle, SetLoggerError> {
    unsafe {
        CURRENT_LOG_LEVEL = level;
        let groups: std::vec::Vec<String> = groups.iter().map(|x| x.to_string()).collect();
        CURRENT_LOG_GROUPS = Some(groups.into_iter().collect());
    }

    let stderr = ConsoleAppender::builder().target(Target::Stderr)
        .encoder(Box::new(PatternEncoder::new("{l} - {m}\n")))
        .build();

    // let logfile = FileAppender::builder()
    //     // Pattern: https://docs.rs/log4rs/*/log4rs/encode/pattern/index.html
    //     .encoder(Box::new(PatternEncoder::new("{d(%Y-%m-%d %H:%M:%S)} {l} - {m}\n")))
    //     .build(std::path::Path::new(&file_path.into()))
    //     .unwrap();

    // Log Trace level output to file where trace is the default level
    // and the programmatically specified level to stderr.
    let config = Config::builder()
        .appender(
            Appender::builder().build("stderr", Box::new(stderr)),
        )
        .build(
            Root::builder().appender("stderr").build(LevelFilter::Trace),
        )
        .unwrap();

    let _handle = log4rs::init_config(config)?;
    Ok(_handle)
}

pub fn __private_log(
    args: std::fmt::Arguments,
    level: log::Level,
    &(group, _module_path, file, line): &(&str, &'static str, &'static str, u32),
) {
    // let fmt = std::format!("{}", args);
    log::log!(level, "{}:{} [{},{}] - {}", file, line, group, 
        unsafe {windows::Win32::System::Threading::GetCurrentProcessId() },
        args);
}

#[macro_export(local_inner_macros)]
macro_rules! trace {
    // trace!("init", LogLevel::Debug, "a {} event", "log")
    ($group:expr, $lvl:expr, $($arg:tt)+) => ({
        #[allow(unused_unsafe)]
        unsafe {
            if CURRENT_LOG_LEVEL >= $lvl && CURRENT_LOG_GROUPS.is_some() && CURRENT_LOG_GROUPS.as_ref().unwrap().contains($group) {
                __private_log(
                    std::format_args!($($arg)+),
                    $lvl,
                    &($group, std::module_path!(), std::file!(), std::line!()),
                );
            }
        }
    });
}

#[macro_export]
macro_rules! __log_format_args {
    ($($args:tt)*) => {
        format_args!($($args)*)
    };
}
