macro_rules! default_max_retries {
    () => {
        5
    };
}

macro_rules! default_thread_sleep {
    ($ms:expr) => {
        |_| ::std::thread::sleep(::core::time::Duration::from_millis($ms))
    };
}

macro_rules! default_tokio_sleep {
    ($ms:expr) => {
        ::tokio::time::sleep(::core::time::Duration::from_millis($ms)).await
    };
}

// TODO: move this to some common util and make this usable outside tokio
macro_rules! retry_inner {
    ($max_retries:expr, $retriable:expr, $calc_sleep_duration:expr, $wait:expr) => {{
        let mut attempts = 0;
        loop {
            match $retriable {
                Ok(val) => break Ok(val),
                Err(err) => {
                    attempts += 1;
                    if attempts >= $max_retries {
                        break Err(err);
                    }
                    $wait($calc_sleep_duration(attempts));
                }
            }
        }
    }};
}

const DEFAULT_MAX_RETRIES: usize = 5;

macro_rules! retry {
    ($retriable:expr) => {
        retry_inner!(
            DEFAULT_MAX_RETRIES,
            $retriable,
            |_| 1000,
            std::thread::sleep
        )
    };
}

macro_rules! retry_tokio {
    ($retriable:expr) => {
        retry_inner!(
            default_max_retries!(),
            $retriable,
            |_| ::std::time::Duration::from_millis(1000),
            default_tokio_sleep
        )
    };
}

#[cfg(test)]
mod tests {
    use std::time::Duration;

    use tokio;

    use super::*;

    #[test]
    fn tesf_foo() {
        let mut x = 5;
        let mut check = || {
            x -= 1;
            eprintln!("check {}", x);
            if x < 0 {
                Err("bad")
            } else {
                Ok("good")
            }
        };

        let res = retry!(check());

        eprintln!("{:?}", res);
    }

    // #[tokio::test]
    // async fn test_async() {
    //     let mut x = 5;
    //     let mut check = || {
    //         x -= 1;
    //         eprintln!("check {}", x);
    //         if x < 0 {
    //             Err("bad")
    //         } else {
    //             Ok("good")
    //         }
    //     };

    //     let res = retry_tokio!(check());

    //     eprintln!("{:?}", res);
    // }
}

pub(crate) use default_max_retries;
pub(crate) use default_thread_sleep;
pub(crate) use default_tokio_sleep;
pub(crate) use retry;
pub(crate) use retry_tokio;
