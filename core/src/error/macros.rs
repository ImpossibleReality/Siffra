#[macro_export]
macro_rules! siffra_error {
    ($message:expr, $span:expr) => {{
        use crate::error::SiffraExecutionError;
        if let Some(span) = $span {
            SiffraExecutionError::new($message.to_string())
                .with_location(file!().to_string(), line!())
                .with_span(span.start(), span.end())
        } else {
            SiffraExecutionError::new($message.to_string())
                .with_location(file!().to_string(), line!())
        }
    }};
    ($message:expr, $description:expr, $span:expr) => {{
        use crate::error::SiffraExecutionError;
        if let Some(span) = $span {
            SiffraExecutionError::new($message.to_string())
                .with_description($description.to_string())
                .with_location(file!().to_string(), line!())
                .with_span(span.start(), span.end())
        } else {
            SiffraExecutionError::new($message.to_string())
                .with_description($description.to_string())
                .with_location(file!().to_string(), line!())
        }
    }};
}

#[macro_export]
macro_rules! siffra_try {
 ($expr:expr, $($arg: expr),* )=> {{
     use crate::siffra_error;
        match $expr {
            Ok(val) => val,
            Err(_) => return Err(siffra_error!($($arg),*)),
        }
    }};
}
