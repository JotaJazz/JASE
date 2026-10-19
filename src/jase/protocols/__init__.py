import re

_CONTROL_RE = re.compile(r"[\x00-\x1f\x7f]")


class ProtocolError(Exception):
    """Respuesta de servidor malformada o inesperada."""


def strip_control(text: str) -> str:
    """Quita caracteres de control ASCII (\\x00-\\x1f, \\x7f) de nombres."""
    return _CONTROL_RE.sub("", text)
