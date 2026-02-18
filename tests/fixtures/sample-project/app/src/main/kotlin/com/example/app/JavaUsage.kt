package com.example.app

import com.example.core.JavaHelper
import com.example.core.User

class JavaUsage {
    private val helper = JavaHelper("app")

    fun createDefaultUser(): User {
        return helper.createUser("Default", "default@example.com")
    }

    fun printNames(users: List<User>) {
        val names = helper.getUserNames(users)
        for (name in names) {
            println(name)
        }
    }
}
