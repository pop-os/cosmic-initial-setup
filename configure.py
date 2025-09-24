#!/usr/bin/env python3

import argparse
from pathlib import Path

def get_parser():
    parser = argparse.ArgumentParser(description='Configure script for Cosmic packages')

    # Build options
    build_opt = parser.add_argument_group('build', 'Build options')
    build_opt.add_argument('-j', '--jobs', default='0', type=int,
                            help='Maximum number of concurrent jobs while building (0 means use all cores) (default: %(default)s)')

    # Layout options
    layout_opt = parser.add_argument_group('layout', 'Package layout options')
    layout_opt.add_argument('--bindir', default='/usr/bin', help='Directory where user-invoked binaries should be installed (default: %(default)s)')
    layout_opt.add_argument('--sbindir', default='/usr/sbin', help='Directory where superuser-invoked binaries should be installed (default: %(default)s)')
    layout_opt.add_argument('--libexecdir', default='/usr/libexec', help='Directory where binaries that will be invoked by other applications should be installed (default: %(default)s)')
    layout_opt.add_argument('--libdir', default='/usr/lib', help='Directory where libraries should be installed (default: %(default)s)')
    layout_opt.add_argument('--includedir', default='/usr/include', help='Directory where header files should be installed (default: %(default)s)')
    layout_opt.add_argument('--datadir', default='/usr/share', help='Directory where data files should be installed (default: %(default)s)')
    layout_opt.add_argument('--mandir', default='/usr/share/man', help='Directory where man pages be installed (default: %(default)s)')
    layout_opt.add_argument('--polkitdir', default='/usr/share/polkit-1', help='Directory where polkit configuration should be installed (default: %(default)s)')
    layout_opt.add_argument('--systemddir', default='/usr/lib/systemd', help='Directory where systemd service files and units should be installed (default: %(default)s)')
    layout_opt.add_argument('--sysconfdir', default='/etc', help='Directory where system config files should be installed (default: %(default)s)')
    layout_opt.add_argument('--xdgdir', default='/etc/xdg', help='Directory where XDG data files be installed (default: %(default)s)')
    layout_opt.add_argument('--pamconfdir', default='/etc/pam.d', help='Directory where PAM configuration should be installed (default: %(default)s)')
    layout_opt.add_argument('--statedir', default='/var/lib', help='Directory where system state should be stored (default: %(default)s)')

    # Install options
    install_opt = parser.add_argument_group('install', 'Package installation options')
    install_opt.add_argument('--installdir', default='/', help='Root directory where all files should be installed (for packaging) (default: %(default)s)')

    return parser

distro_conf_contents = \
"""### Managed by configure.py script, do not manually edit
"""

if __name__ == '__main__':
    parser = get_parser()
    args = parser.parse_args()

    for key, var in vars(args).items():
        distro_conf_contents += "DISTRO_" + key.upper() + "=\"" + str(var) + "\"\n"

    # Get the path to this script
    root_dir = Path(__file__).parent
    distro_conf_f = root_dir / ".distro_config.env"

    with open(distro_conf_f, "w") as f:
        f.write(distro_conf_contents)

    print(".distro_config.env successfully generated")
