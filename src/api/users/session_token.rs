use std::{fmt, time::SystemTime};

use rand::{distributions::Alphanumeric, thread_rng, Rng};
use serde::{Deserialize, Serialize};

#[derive(Deserialize, Serialize, Clone, Debug, Hash, PartialEq, Eq)]
pub struct SessionToken(String);

impl SessionToken {
    pub fn new() -> SessionToken {
        SessionToken(
            thread_rng()
                .sample_iter(&Alphanumeric)
                .take(30)
                .map(char::from)
                .collect::<String>()
                + "##"
                + &SystemTime::now()
                    .duration_since(SystemTime::UNIX_EPOCH)
                    .unwrap()
                    .as_secs()
                    .to_string(),
        )
    }

    pub fn parse(text: &str) -> SessionToken {
        // const SESSION_TOKEN_LEN: usize = 42;
        // if text.len() >= SESSION_TOKEN_LEN {
        //     Some(SessionToken(text.to_string()))
        // } else {
        //     None
        // }
        SessionToken(text.to_string())
    }
}
impl fmt::Display for SessionToken {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}
