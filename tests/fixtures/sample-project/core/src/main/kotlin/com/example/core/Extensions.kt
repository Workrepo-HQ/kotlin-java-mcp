package com.example.core

fun User.displayName(): String {
    return "$name <$email>"
}

fun String.isValidEmail(): Boolean {
    return this.contains("@") && this.contains(".")
}

val User.isAdmin: Boolean
    get() = role == UserRole.ADMIN
