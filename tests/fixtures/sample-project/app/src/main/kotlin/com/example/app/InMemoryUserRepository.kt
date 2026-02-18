package com.example.app

import com.example.core.Repository
import com.example.core.User

class InMemoryUserRepository : Repository<User> {
    private val storage = mutableMapOf<String, User>()

    override fun findById(id: String): User? = storage[id]

    override fun findAll(): List<User> = storage.values.toList()

    override fun save(entity: User): User {
        storage[entity.id] = entity
        return entity
    }

    override fun delete(id: String) {
        storage.remove(id)
    }
}
