pub struct Auth {
    pub access_token: String,
    pub uuid: String,
    pub username: String,
    pub user_type: String,
    pub user_properties: String,
}

impl Auth {
    /// Create a new Auth object
    /// # Arguments
    /// * `user_type` - The type of user (mojang or msa)
    /// * `user_properties` - The user properties (json string)
    /// * `username` - The username of the user
    /// * `uuid` - The uuid of the user
    /// * `access_token` - The access token of the user
    pub fn new(
        user_type: String,
        user_properties: String,
        username: String,
        uuid: String,
        access_token: String,
    ) -> Self {
        Self {
            user_type,
            user_properties,
            username,
            uuid,
            access_token,
        }
    }
}

impl Default for Auth {
    fn default() -> Self {
        Self {
            access_token: "636da1d35e803b00aae0fcd8333f9234".to_string(),
            uuid: "636da1d35e803b00aae0fcd8333f9234".to_string(),
            username: "Player".to_string(),
            user_type: "mojang".to_string(),
            user_properties: "{}".to_string(),
        }
    }
}

pub struct OfflineAuth {
    pub username: String,
}

impl OfflineAuth {
    /// Create an offline Auth object from a username
    /// # Arguments
    /// * `username` - The username of the user
    /// # Example
    /// ```
    /// let auth = OfflineAuth::new("Player");
    /// ```
    pub fn new(username: &str) -> Auth {
        let mut uuid = md5::compute(username.as_bytes());
        uuid[6] &= 0x0f;
        uuid[6] |= 0x30;
        uuid[8] &= 0x3f;
        uuid[8] |= 0x80;
        let uuid = uuid
            .iter()
            .map(|b| format!("{:02x}", b))
            .collect::<String>();

        Auth {
            access_token: uuid.clone(),
            uuid,
            username: username.to_string(),
            user_type: "mojang".to_string(),
            user_properties: "{}".to_string(),
        }
    }
}
