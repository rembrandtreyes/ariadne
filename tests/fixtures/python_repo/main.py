from utils import helper

def greet(name):
    return helper(f"Hello, {name}")

class UserService:
    def get_user(self, user_id):
        return {"id": user_id, "name": "Alice"}

    def create_user(self, name):
        return {"name": name}
