from flask import Flask

app = Flask(__name__)

@app.route("/api/users")
def get_users():
    return {"users": []}
