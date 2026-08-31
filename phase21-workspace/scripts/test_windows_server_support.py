"""Hermetic checks for the Windows Server admission and feature contract."""

from pathlib import Path
import xml.etree.ElementTree as ET

ROOT = Path(__file__).resolve().parents[1]
product = (ROOT / "installer/wix/Product.wxs").read_text(encoding="utf-8")
bundle = (ROOT / "installer/wix/Bundle.wxs").read_text(encoding="utf-8")
ET.parse(ROOT / "installer/wix/Product.wxs")
ET.parse(ROOT / "installer/wix/Bundle.wxs")


def admitted(product_type: int, build: int) -> bool:
    return (product_type == 1 and build >= 22621) or (product_type == 3 and build >= 26100)


assert admitted(1, 22621)
assert admitted(3, 26100)
assert not admitted(3, 20348)
assert not admitted(2, 26100)
assert 'MsiNTProductType = 1 AND OSCURRENTBUILD &gt;= 22621' in product
assert 'MsiNTProductType = 3 AND OSCURRENTBUILD &gt;= 26100' in product
assert 'NTProductType = 3 AND WindowsBuildNumber &gt;= 26100' in bundle
assert 'Name="InstallationType"' in product
assert 'WINDOWSINSTALLATIONTYPE ~= &quot;Server Core&quot;' in product
assert 'InstallCondition="NOT (WindowsInstallationType ~= &quot;Server Core&quot;)"' in bundle
print("windows server support checks: PASS")
