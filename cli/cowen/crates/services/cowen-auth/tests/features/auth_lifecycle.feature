Feature: Auth Lifecycle and Token Refresh
  As a cowen authentication core
  I want to fetch and automatically renew tokens via HttpSender
  So that the gateway and other components always have valid tokens

  Scenario: OAuth2 token fetch and refresh successful
    Given the AuthMode is "OAuth2"
    And the HttpSender will return a valid token with expires_in 3600
    When I initialize the AuthClient and request a token
    Then the returned token should be valid
    And the HttpSender should have been called 1 time

  Scenario: SelfBuilt token fallback to vault on 401
    Given the AuthMode is "SelfBuilt"
    And the HttpSender will return a 401 Unauthorized
    When I initialize the AuthClient and request a token
    Then it should fall back to vault or throw expected diagnostics error
    And the HttpSender should have been called 1 time
