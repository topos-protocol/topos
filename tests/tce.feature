Feature: TCE
    Background:
        Given TCE network of "1000" <nodes>
        And certificates <cert> being part of history
        And Alice being "node[10]"
        And Bob being "node[200]"

    Scenario Outline: Die and retry
        Given history of Alice is corrupted
        When Alice restart # including initial synchronization of history
        Then Alice history is equal to the history of Bob

    Scenario Outline: Late at the party
        Given Alice shutdown
        When Bob gossip <certificate> to neighbors
        And waiting <n> seconds
        And Alice restart
        And waiting <k> seconds
        Then history of all <nodes> must delivered <certificate>

    Scenario Outline: Minimal robustness to crash fault
        When Alice gossip <certificate> to neighbors
        And waiting <k> seconds # just the time for nodes to engage in samples
        And <nb> nodes out of <nodes> shutdown
        Then history of all <nodes> except <nb> must delivered <certificate>

    Scenario Outline: Gossip
        When Alice gossip <certificate> to neighbors
        And waiting <n> seconds
        # If not at least one more: cover failure from Alice
        Then at least two in <nodes> must have <certificate> in their queue
        # If not all of them: cover broader failure from network topology
        And all <nodes> must have <certificate> in their queue

    Scenario Outline: Totality
        When Alice gossip <certificate> to neighbors
        And waiting <n> seconds
        Then history of all <nodes> must delivered <certificate>

    # Conflicting certificates not there for Q3
    Scenario Outline: Consistency
        When Alice gossip conflicting <certificate_A1> and <certificate_A2>
        And waiting <n> seconds
        Then Bob history includes either <certificate_A1> or <certificate_A2>
        And history of all <nodes> must be equal to the history of Bob
