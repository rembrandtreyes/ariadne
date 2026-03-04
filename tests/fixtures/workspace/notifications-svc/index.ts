export async function sendNotification(userId: string, message: string): Promise<void> {
    console.log(`Sending to ${userId}: ${message}`);
}

export async function fetchUserEmail(userId: string): Promise<string> {
    const response = await fetch(`http://localhost:8000/api/users/${userId}`);
    const data = await response.json();
    return data.email;
}
