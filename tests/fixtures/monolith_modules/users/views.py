from .models import User

def get_user(user_id):
    return User("Alice", "alice@example.com").to_dict()

def list_users():
    return [get_user(1)]
