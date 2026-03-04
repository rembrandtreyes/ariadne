import requests

def process_payment(user_id, amount):
    return {"status": "ok", "user_id": user_id, "amount": amount}

def get_user_payments(user_id):
    response = requests.get(f"http://localhost:8000/api/users/{user_id}")
    return response.json()
