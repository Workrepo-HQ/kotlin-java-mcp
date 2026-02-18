package com.example.core;

import java.util.List;
import java.util.ArrayList;

/**
 * A Java helper class that references the Kotlin-defined User class.
 */
public class JavaHelper {
    private String prefix;

    public JavaHelper(String prefix) {
        this.prefix = prefix;
    }

    public User createUser(String name, String email) {
        return new User(
            prefix + "-" + System.currentTimeMillis(),
            name,
            email,
            UserRole.EDITOR
        );
    }

    public List<String> getUserNames(List<User> users) {
        List<String> names = new ArrayList<>();
        for (User u : users) {
            names.add(u.getName());
        }
        return names;
    }

    public String getPrefix() {
        return prefix;
    }
}
