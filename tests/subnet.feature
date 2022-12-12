Feature: Subnet
    Background:
        Given a substrate-node "client 0"
        And a topos-node "client 1"
        And a topos-subnet-node "client 2"
        And wallet information for all involved network

    Scenario Outline: Subnet creation
        Given new subnet creation with "<peers>" pioneers, "<genesis>" chainspec
        When the subnet is spawned with "10" peers
        And waiting <dkg_time> blocks from "client 1"
        Then "<subnet_pk>" must be retrievable
        And Each "<peers>" must return "<subnet_pk>"

        Examples:
            # In practice more than one peer
            | peers                                                                    | genesis            | nb_blocks |
            | /ip4/7.7.7.7/tcp/4242/p2p/QmYyQSo1c1Ym7orWxLYvCrM2EmxFTANf8wXmmE7DWjhx5N | { json chainspec } | 10        |

    Scenario Outline: Subnet registration
        Given subnet information <genesis> and "<pk>" created
        And wallet information with fund of <100> TOPOS
        When registration tx with <genesis> and "<pk>" is created
        And registration tx is submitted to "client 2"
        And waiting <k> blocks from "client 2"
        And requesting the list of registered subnet from "client 2"
        Then the list must include "<pk>"