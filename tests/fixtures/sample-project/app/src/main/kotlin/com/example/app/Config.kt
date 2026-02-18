package com.example.app

import com.example.core.UserService

object Config {
    val maxUsers: Int = UserService.MAX_USERS

    fun createUserService(): UserService {
        val repo = InMemoryUserRepository()
        return UserService(repo)
    }
}
