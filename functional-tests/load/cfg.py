from locust import HttpUser


class LoadConfig:
    """
    A data class holding host and job classes that describe load requests.
    """

    user_classes: list[HttpUser]
    host: str
    spawn_rate: int

    def __init__(self, user_classes: list[HttpUser], host: str, spawn_rate: int):
        # "Patch" user_classes with a given host
        for user_cls in user_classes:
            user_cls.host = host

        self.user_classes = user_classes
        self.host = host
        self.spawn_rate = spawn_rate


class LoadConfigBuilder:
    """
    A builder for the load config.
    """

    def __init__(self):
        pass

    def __call__(self, )
