package com.chanjet.connector.server;

import com.chanjet.connector.api.client.IInternalHttpClient;
import org.mockito.Mockito;
import org.springframework.boot.test.context.TestConfiguration;
import org.springframework.context.annotation.Bean;
import org.springframework.context.annotation.Primary;

@TestConfiguration
public class TckTestConfig {

    @Bean
    @Primary
    public IInternalHttpClient mockInternalHttpClient() {
        return Mockito.mock(IInternalHttpClient.class);
    }

}
