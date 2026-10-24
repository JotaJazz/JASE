import pytest

from jase import jase_url
from jase.models import ServerAddress


def test_build_and_roundtrip():
    a = ServerAddress("1.2.3.4", 28960)
    url = jase_url.build("cod2", a)
    assert url == "jase://connect/cod2/1.2.3.4:28960"
    assert jase_url.parse(url) == ("cod2", a)


@pytest.mark.parametrize("url,host", [
    ("jase://connect/cod2/1.2.3.4:28960/", "1.2.3.4"),
    ("JASE://connect/cod2/play.example.com:1", "play.example.com"),
    ("jase://connect/cod2/a-b.c:65535", "a-b.c"),
])
def test_valid(url, host):
    game, addr = jase_url.parse(url)
    assert game == "cod2" and addr.host == host


@pytest.mark.parametrize("url", [
    "jase://connect/cod2/;rm -rf:1",
    "jase://connect/cod2/1.2.3.4;ls:1",
    "jase://connect/nogame/1.2.3.4:1",
    "jase://connect/cod2/1.2.3.4:0",
    "jase://connect/cod2/1.2.3.4:70000",
    "jase://connect/cod2/1.2.3.4:-1",
    "jase://connect/cod2/1.2.3.4:",
    "jase://connect/cod2/1.2.3.4",
    "jase://connect/cod2/" + "a" * 300 + ":1",
    "http://connect/cod2/1.2.3.4:1",
    "jase:connect/cod2/1.2.3.4:1",
    "jase://launch/cod2/1.2.3.4:1",
    "jase://connect/cod2/1.2.3.4:1/extra",
    "jase://connect/cod2/extra/1.2.3.4:1",
    "jase://connect/1.2.3.4:1",
    "jase://user@1.2.3.4/cod2/x:1",
    "jase://connect/cod2/user@1.2.3.4:1",
    "jase://connect/cod2/1.2.3.4:1?x=1",
    "jase://connect/cod2/1.2.3.4:1#frag",
    "jase://connect/cod2/:28960",
    "jase://connect/cod2/-.:1",
    "jase://connect/cod2/ho st:1",
    "jase://connect/cod2/hóst:1",
    "jase://connect/cod2/1.2.3.4:١",
    "jase://connect/cod2/1.2.3.4:1\n",
    "jase://connect/cod2/1.2.3.4:1\x00",
    "jase://connect/cod2/[::1]:1",
    "",
])
def test_rejected(url):
    assert jase_url.parse(url) is None
