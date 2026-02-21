import asyncio

from .app import run


def main() -> None:
    asyncio.run(run())


if __name__ == "__main__":
    main()
