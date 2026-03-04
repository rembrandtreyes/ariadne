public class Main {
    public static void main(String[] args) {
        greet("World");
    }

    public static String greet(String name) {
        return "Hello, " + name;
    }
}

class UserService {
    public String getUser(int id) {
        return "user_" + id;
    }
}
