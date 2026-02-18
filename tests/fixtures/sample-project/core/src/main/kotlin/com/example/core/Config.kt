package com.example.core

object Config {
    val maxRetries = 3
    fun getTimeout(): Long = 5000L
}

infix fun String.mapTo(other: String): Pair<String, String> = Pair(this, other)

fun createUser(name: String): User = User(name, "$name@example.com", UserRole.ADMIN)
