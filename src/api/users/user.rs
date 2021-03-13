use crate::{database::DatabaseManager, game::client_adapter::ClientAdapter};
use actix::Addr;
use rand::{thread_rng, Rng};
use serde::{de, Deserialize, Serialize, Serializer};
use std::fmt;

const USER_ID_LEN: usize = 12;
const VALID_USER_ID_CHARS: &str = "0123456789abcdef";

#[derive(Clone, Copy, Eq, Hash)]
pub struct UserId([char; USER_ID_LEN]);

impl UserId {
    pub fn new() -> UserId {
        //users: &[User]
        // let mut id =
        Self::generate_inner()
        // while users.iter().any(|u| u.id == id) {
        //     id = Self::generate_inner();
        // }
        // id
    }
    fn generate_inner() -> UserId {
        let abc = VALID_USER_ID_CHARS.chars().collect::<Vec<_>>();
        // let mut rand_chars: [char; USER_ID_LEN] = ['a'; USER_ID_LEN];
        let mut rand_id = ['0'; USER_ID_LEN];
        for rand_char in rand_id.iter_mut() {
            // *rand_char = abc[thread_rng().gen()];
            // *rand_digit =
            *rand_char = abc[thread_rng().gen_range(0, VALID_USER_ID_CHARS.len())];
        }
        // thread_rng().fill(&mut rand_id);
        UserId(rand_id)
    }
    pub fn from_str(s: &str) -> Result<UserId, &str> {
        let s = s.to_lowercase();
        let mut inner: [char; USER_ID_LEN] = ['0'; USER_ID_LEN];
        if s.len() != USER_ID_LEN {
            Err("Could not deserialize UserId")
        } else {
            for (i, c) in s.chars().enumerate() {
                if VALID_USER_ID_CHARS.contains(c) {
                    inner[i] = c;
                } else {
                    return Err("Invalid character");
                }
            }
            Ok(UserId(inner))
        }
    }
}
impl fmt::Display for UserId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        use std::fmt::Write;
        self.0.iter().for_each(|c| {
            let _ = f.write_char(*c);
        });

        fmt::Result::Ok(())
    }
}
impl fmt::Debug for UserId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        use std::fmt::Write;
        self.0.iter().for_each(|c| {
            let _ = f.write_char(*c);
        });

        fmt::Result::Ok(())
    }
}
impl PartialEq for UserId {
    fn eq(&self, other: &Self) -> bool {
        self.to_string() == other.to_string()
    }
}
impl Serialize for UserId {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        serializer.serialize_str(&self.to_string())
    }
}
impl<'de> Deserialize<'de> for UserId {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: de::Deserializer<'de>,
    {
        let s: String = de::Deserialize::deserialize(deserializer)?;
        UserId::from_str(&s).map_err(de::Error::custom)
    }
}

const MIN_PASSWORD_LENGTH: usize = 6;
const MAX_PASSWORD_LENGTH: usize = 15;
const SPECIAL_CHARS: &str = "0123456789=!<[>]()-/{}~+%$|#';&+€";
const INVALID_CHARS: &str = "#:\\\"";

#[derive(Clone)]
pub struct BackendUser {
    // #[serde(deserialize_with = "UserId::deserialize")]
    pub id: UserId,
    pub username: String,
    pub password: HashedPassword,
    pub email: Option<String>,
    pub game_info: UserGameInfo,
    pub friends: Vec<UserId>,
    pub playing: Option<Addr<ClientAdapter>>,
    pub friend_requests: Vec<BackendFriendRequest>,
}
impl BackendUser {
    pub fn new(username: String, password: String) -> BackendUser {
        BackendUser {
            id: UserId::new(),
            username,
            password: HashedPassword::new(password),
            email: None,
            game_info: UserGameInfo::new(),
            friends: Vec::new(),
            playing: None,
            friend_requests: Vec::new(),
            // login_key: None,
        }
    }

    pub fn check_password(password: &str) -> bool {
        password.len() > MIN_PASSWORD_LENGTH
            && password.chars().any(|c| SPECIAL_CHARS.contains(c))
            && password.len() < MAX_PASSWORD_LENGTH
            && !password.chars().any(|c| INVALID_CHARS.contains(c))
    }

    pub fn gen_new_id(&mut self) {
        self.id = UserId::new();
    }
}
pub use pw::*;

pub mod pw {
    use serde::{de, Deserialize, Serialize, Serializer};
    use sha3::{Digest, Keccak256};
    use std::fmt;

    #[derive(Debug, Clone, PartialEq)]
    pub struct HashedPassword(Vec<u8>);
    impl HashedPassword {
        pub fn new(password: String) -> HashedPassword {
            HashedPassword(Self::hash(&password))
        }
        fn hash(string: &str) -> Vec<u8> {
            Keccak256::digest(string.as_bytes()).into_iter().collect()
        }
        pub fn matches(&self, password: &str) -> bool {
            self.0 == Self::hash(password)
        }

        fn from_str(string: &str) -> Result<HashedPassword, &str> {
            let mut vec = Vec::new();
            for i in (0..string.len()).step_by(2) {
                if let Ok(b) = u8::from_str_radix(&string[i..i + 2], 16) {
                    vec.push(b);
                } else {
                    return Err("Invalid hex byte");
                }
            }
            Ok(HashedPassword(vec))
        }
    }

    impl fmt::Display for HashedPassword {
        fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
            for h in self.0.iter() {
                write!(f, "{:02x}", h)?;
            }
            fmt::Result::Ok(())
        }
    }

    impl Serialize for HashedPassword {
        fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
        where
            S: Serializer,
        {
            serializer.serialize_str(&self.to_string())
        }
    }
    impl<'de> Deserialize<'de> for HashedPassword {
        fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
        where
            D: de::Deserializer<'de>,
        {
            let s: String = de::Deserialize::deserialize(deserializer)?;
            HashedPassword::from_str(&s).map_err(de::Error::custom)
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UserGameInfo {
    pub skill_rating: i32,
    // pub rank: u32,
}
impl UserGameInfo {
    fn new() -> UserGameInfo {
        UserGameInfo { skill_rating: 1000 }
    }
}

#[derive(Serialize, Deserialize)]
pub struct PlayedGameInfo {
    pub winner: UserId,
    pub loser: UserId,
    // pub one_won: bool,
    // kind: GameKind,
}
impl PlayedGameInfo {
    pub fn new(winner: UserId, loser: UserId) -> PlayedGameInfo {
        PlayedGameInfo { winner, loser }
    }
}
#[derive(Serialize, Deserialize)]
pub enum GameKind {
    Ranked,
    Simple,
}

#[derive(Clone, Serialize, Deserialize)]
pub struct PublicUserMe {
    pub id: UserId,
    pub username: String,
    pub email: Option<String>,
    pub game_info: UserGameInfo,
    pub friends: Vec<Friend>,
    // pub friend_requests: Vec<PublicFriendRequest>,
    // pub playing: bool,
}
impl PublicUserMe {
    pub fn from(user: BackendUser, db: &DatabaseManager) -> Self {
        Self {
            id: user.id,
            username: user.username,
            email: user.email,
            game_info: user.game_info,
            friends: user
                .friends
                .into_iter()
                .filter_map(|friend_id| Friend::from(friend_id, PublicFriendState::IsFriend, &db))
                .chain(user.friend_requests.into_iter().filter_map(|friend_req| {
                    let friend_state = match friend_req.direction {
                        BackendFriendRequestDirection::Incoming => {
                            PublicFriendState::HasRequestedMe
                        }
                        BackendFriendRequestDirection::Outgoing => {
                            PublicFriendState::IsRequestedByMe
                        }
                    };
                    Friend::from(friend_req.other_id, friend_state, &db)
                }))
                .collect::<Vec<_>>(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PublicUserOther {
    pub id: UserId,
    pub username: String,
    pub game_info: UserGameInfo,
    pub playing: bool,
}
impl PublicUserOther {
    pub fn from(user: BackendUser) -> Self {
        Self {
            id: user.id,
            username: user.username,
            game_info: user.game_info,
            playing: user.playing.is_some(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BackendFriendRequest {
    pub direction: BackendFriendRequestDirection,
    pub other_id: UserId,
}

#[derive(PartialEq, Debug, Clone, Serialize, Deserialize)]
pub enum BackendFriendRequestDirection {
    Incoming,
    Outgoing,
}

#[derive(Clone, Serialize, Deserialize)]
pub enum PublicFriendState {
    IsFriend,
    IsRequestedByMe,
    HasRequestedMe,
    None,
}

#[derive(Clone, Serialize, Deserialize)]
pub struct Friend {
    id: UserId,
    username: String,
    game_info: UserGameInfo,
    friend_state: PublicFriendState,
    playing: bool,
}
impl Friend {
    pub fn from(
        friend_id: UserId,
        friend_state: PublicFriendState,
        db: &DatabaseManager,
    ) -> Option<Self> {
        db.users
            .get_id(&friend_id, &db.friend_requests)
            .map(|user| Friend {
                id: user.id,
                username: user.username,
                game_info: user.game_info,
                friend_state,
                playing: user.playing.is_some(),
            })
    }
}
