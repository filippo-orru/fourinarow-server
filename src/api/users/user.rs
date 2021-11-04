use actix::Addr;
use rand::{thread_rng, Rng};
use serde::{de, Deserialize, Serialize, Serializer};
use std::fmt;
use std::slice::Iter;

use crate::api::chat::ChatThreadId;
use crate::database::DatabaseManager;
use crate::game::client_state::ClientState;

const USER_ID_LEN: usize = 12;
const VALID_USER_ID_CHARS: &str = "0123456789abcdef";

#[derive(Clone, Copy, Eq, Hash, PartialOrd)]
pub struct UserId([char; USER_ID_LEN]);

impl Ord for UserId {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        self.to_string().cmp(&other.to_string())
    }
}

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
        f.write_char('"')?;
        self.0.iter().for_each(|c| {
            let _ = f.write_char(*c);
        });
        f.write_char('"')?;

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
pub struct BackendUserMe {
    pub id: UserId,
    pub username: String,
    pub password: HashedPassword,
    pub email: Option<String>,
    pub game_info: UserGameInfo,
    pub playing: Option<Addr<ClientState>>,
    pub friendships: BackendFriendshipsMe,
}

impl BackendUserMe {
    pub fn new(username: String, password: String) -> BackendUserMe {
        BackendUserMe {
            id: UserId::new(),
            username,
            password: HashedPassword::new(password),
            email: None,
            game_info: UserGameInfo::new(),
            playing: None,
            friendships: BackendFriendshipsMe::new(),
        }
    }

    pub fn to_public_user_me(self, db: &DatabaseManager) -> PublicUserMe {
        PublicUserMe {
            id: self.id,
            username: self.username,
            email: self.email,
            game_info: self.game_info,
            friendships: self.friendships.to_public(db),
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

#[derive(Debug, Clone)]
pub struct BackendFriendshipsMe(Vec<BackendFriendshipMe>);

impl BackendFriendshipsMe {
    pub fn new() -> BackendFriendshipsMe {
        BackendFriendshipsMe(Vec::new())
    }

    pub fn from(v: Vec<BackendFriendshipMe>) -> Self {
        BackendFriendshipsMe(v)
    }

    pub fn friends(&self) -> impl Iterator<Item = &BackendFriendshipMe> {
        self.0.iter().filter(|f| {
            if let BackendFriendshipState::Friends { chat_thread_id: _ } = f.state {
                true
            } else {
                false
            }
        })
    }

    pub fn iter(&self) -> Iter<BackendFriendshipMe> {
        self.0.iter()
    }

    fn to_public(self, db: &DatabaseManager) -> Vec<PublicFriend> {
        self.iter()
            .filter_map(|friendship| -> Option<PublicFriend> {
                let chat_thread_id = if let BackendFriendshipState::Friends { chat_thread_id } =
                    friendship.state.clone()
                {
                    Some(chat_thread_id)
                } else {
                    None
                };

                db.users
                    .get_id_public(&friendship.other_id)
                    .map(|user| PublicFriend {
                        user,
                        friend_state: friendship.state.to_public(),
                        chat_thread_id,
                    })
            })
            .collect()
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

#[derive(Serialize, Deserialize, Debug)]
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

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct PublicUserMe {
    pub id: UserId,
    pub username: String,
    pub email: Option<String>,
    pub game_info: UserGameInfo,
    pub friendships: Vec<PublicFriend>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct PublicFriend {
    pub user: PublicUserOther,
    pub friend_state: PublicFriendState,
    pub chat_thread_id: Option<ChatThreadId>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PublicUserOther {
    pub id: UserId,
    pub username: String,
    pub game_info: UserGameInfo,
    pub playing: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BackendFriendshipMe {
    pub state: BackendFriendshipState,
    pub other_id: UserId,
}

#[derive(PartialEq, Debug, Clone, Serialize, Deserialize)]
pub enum BackendFriendshipState {
    ReqIncoming,
    ReqOutgoing,
    Friends { chat_thread_id: ChatThreadId },
}

impl BackendFriendshipState {
    fn to_public(&self) -> PublicFriendState {
        use BackendFriendshipState::*;
        match self {
            ReqOutgoing => PublicFriendState::IsRequestedByMe,
            ReqIncoming => PublicFriendState::HasRequestedMe,
            Friends { chat_thread_id: _ } => PublicFriendState::IsFriend,
        }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub enum PublicFriendState {
    IsFriend,
    IsRequestedByMe,
    HasRequestedMe,
}
