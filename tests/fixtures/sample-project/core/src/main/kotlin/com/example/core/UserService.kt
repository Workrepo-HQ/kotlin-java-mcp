package com.example.core

class UserService(private val repository: Repository<User>) {

    fun getUser(id: UserId): User? {
        return repository.findById(id)
    }

    fun getAllUsers(): List<User> {
        return repository.findAll()
    }

    fun createUser(name: String, email: String, role: UserRole): User {
        val user = User(
            id = generateId(),
            name = name,
            email = email,
            role = role
        )
        return repository.save(user)
    }

    fun deleteUser(id: UserId) {
        repository.delete(id)
    }

    companion object {
        const val MAX_USERS = 1000

        fun generateId(): String {
            return java.util.UUID.randomUUID().toString()
        }
    }
}
