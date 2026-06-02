"""
tidemark: snapshot a directory tree and diff what changed - no git required.
"""

try:
    from importlib.metadata import version
    __version__ = version("tidemark")
except ImportError:
    from importlib_metadata import version
    __version__ = version("tidemark")
