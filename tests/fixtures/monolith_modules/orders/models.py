class Order:
    def __init__(self, user_id, total):
        self.user_id = user_id
        self.total = total

    def to_dict(self):
        return {"user_id": self.user_id, "total": self.total}
