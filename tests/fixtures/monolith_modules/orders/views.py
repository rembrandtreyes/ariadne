from .models import Order
from users.models import User

def create_order(user_id, total):
    order = Order(user_id, total)
    return order.to_dict()
