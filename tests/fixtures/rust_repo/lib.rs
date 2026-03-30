pub fn greet(name: &str) -> String {
    format!("Hello, {}", name)
}

pub fn welcome(name: &str) -> String {
    let greeting = greet(name);
    format!("{} Welcome!", greeting)
}

pub struct UserService;

impl UserService {
    pub fn new() -> Self {
        UserService
    }

    pub fn get_user(&self, id: u64) -> String {
        format!("User {}", id)
    }

    pub fn greet_user(&self, id: u64) -> String {
        let name = self.get_user(id);
        greet(&name)
    }
}

fn helper() -> UserService {
    UserService::new()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_greet() {
        let result = greet("world");
        assert_eq!(result, "Hello, world");
    }

    #[test]
    fn test_welcome() {
        let result = welcome("world");
        assert!(result.contains("Welcome"));
    }
}
