package com.example.core

data class User(
    val id: String,
    val name: String,
    val email: String,
    val role: UserRole
)

enum class UserRole {
    ADMIN,
    EDITOR,
    VIEWER
}

typealias UserId = String
