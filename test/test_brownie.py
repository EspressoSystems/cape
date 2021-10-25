from brownie import accounts


def test_token_deploys(Greeter):
    token = accounts[0].deploy(Greeter, "Hello pytest!")
    assert token.greet() == "Hello pytest!"
