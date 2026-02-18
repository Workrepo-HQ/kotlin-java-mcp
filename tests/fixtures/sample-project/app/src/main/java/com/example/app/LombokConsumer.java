package com.example.app;

import com.example.core.LombokUser;

public class LombokConsumer {
    public void process(LombokUser user) {
        String name = user.getUsername();
        String mail = user.getEmail();
        boolean isActive = user.isActive();
        String id = user.getId();

        user.setUsername("new-name");
        user.setEmail("new@example.com");
        user.setActive(false);
    }
}
