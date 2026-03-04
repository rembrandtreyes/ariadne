<?php

namespace App\Controllers;

use App\Models\User;
use App\Services\AuthService;

interface Authenticatable {
    public function authenticate(string $password): bool;
}

class UserController implements Authenticatable {
    private AuthService $authService;

    public function __construct(AuthService $authService) {
        $this->authService = $authService;
    }

    public function authenticate(string $password): bool {
        return $this->authService->verify($password);
    }

    public function createUser(string $name): void {
        $user = new User($name);
        $this->validateInput($name);
        $user->save();
    }

    private function validateInput(string $input): void {
        if (empty($input)) {
            throw new \InvalidArgumentException("Input required");
        }
    }
}

enum UserRole: string {
    case Admin = 'admin';
    case User = 'user';
}

function processRequest(): void {
    $controller = new UserController(new AuthService());
    $controller->createUser("Alice");
}
