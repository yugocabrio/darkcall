"""Strict metadata for two compiled native circuits; never bundle-supplied keys."""
import json
import os
import pathlib
import stat

WORKING = 'working-33-column-v1'
SHARED = 'experimental-shared18-native-row-v1'


def read_layout(directory, private_reader=None):
    marker = pathlib.Path(directory) / 'native-layout.json'
    try:
        metadata = marker.lstat()
    except FileNotFoundError:
        return WORKING  # Only genuine absence preserves historical artifacts.
    if not stat.S_ISREG(metadata.st_mode) or metadata.st_size > 4096:
        raise RuntimeError('Bounded regular native layout metadata required')
    if private_reader is not None:
        data = private_reader(marker, 4096)
    else:
        fd = os.open(marker, os.O_RDONLY | os.O_NOFOLLOW)
        with os.fdopen(fd, 'rb') as handle:
            opened = os.fstat(handle.fileno())
            if (not stat.S_ISREG(opened.st_mode)
                    or (opened.st_dev, opened.st_ino) != (metadata.st_dev, metadata.st_ino)):
                raise RuntimeError('Native layout file changed while opening')
            data = handle.read(4097)
    if len(data) > 4096:
        raise RuntimeError('Native layout metadata exceeds bound')
    value = json.loads(data)
    if (not isinstance(value, dict) or set(value) != {'format', 'layout'}
            or type(value['format']) is not int or value['format'] != 1
            or value['layout'] not in (WORKING, SHARED)):
        raise RuntimeError('Unknown/malformed fixed native layout metadata')
    return value['layout']
