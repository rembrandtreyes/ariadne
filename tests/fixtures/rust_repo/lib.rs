pub fn greet(name: &str) -> String {
    format!("Hello, {}", name)
}

pub struct UserService;

impl UserService {
    pub fn get_user(&self, id: u64) -> String {
        format!("User {}", id)
    }
}
