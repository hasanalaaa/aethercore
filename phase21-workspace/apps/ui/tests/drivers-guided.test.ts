// Quality pass (lane 2): the Drivers page tells the user what it did not check, what age a vendor driver
// has, and where to get a driver Windows cannot supply. Read-only: no new action, nothing downloaded.
// 1. A scan that only read the local cache says so, once, with the one button that searches online.
// 2. The inventory loads when the page opens (local and read-only), once.
// 3. Age shows only for vendor-installed drivers (oemNN.inf): Windows' own 2006 drivers are not "old".
// 4. Devices with no driver and no installable offer are grouped by vendor with hardware IDs and the
//    maker's support address as text, never as a link.
// Run: node --experimental-strip-types --import ./tests/resolve-ts.mjs --test tests/*.test.ts
import { test } from 'node:test';
import assert from 'node:assert/strict';
import { hasMessageKey } from '../src/lib/i18n/index.ts';
import { enCatalog } from '../src/lib/i18n/catalog.en.ts';
import { arCatalog } from '../src/lib/i18n/catalog.ar.ts';
import type { DriverDevice } from '../src/lib/contracts.ts';

const arabic = /[؀-ۿ]/;
const NOW = Date.UTC(2026, 9, 10);

const device = (over: Partial<DriverDevice>): DriverDevice => ({
  instanceId: 'X\\0', displayName: 'Device', description: '', className: 'System', classGuid: '', manufacturer: '', enumerator: '', location: '',
  hardwareIds: [], compatibleIds: [], rawStatus: 0, problemCode: 0, hasProblem: false, missingDriver: false, driver: null, deviceState: '', gpu: null,
  displayManaged: false, recommendedCandidateId: '', updateStatus: '', authorityCoverage: '', candidates: [], requiredAuthorities: [], evaluatedAuthorities: [],
  unavailableAuthorities: [], unsupportedAuthorities: [], manualAuthorities: [], managementAuthorities: [], ...over,
}) as DriverDevice;
const withDriver = (name: string, infPath: string, date: string, provider = 'Intel') =>
  device({ displayName: name, driver: { provider, version: '1.0', infPath, date } });

const KEYS: Record<string, string[]> = {
  'drivers.notSearched.title': [],
  'drivers.notSearched.bodyDate': ['date'],
  'drivers.notSearched.bodyUnknown': [],
  'drivers.manual.title': [],
  'drivers.manual.group': ['count', 'vendor'],
  'drivers.manual.groupUnknown': ['count'],
  'drivers.manual.body': ['source'],
  'drivers.manual.sourceMachine': ['address', 'machine'],
  'drivers.manual.sourceVendor': ['address', 'vendor'],
  'drivers.manual.sourceGeneric': [],
  'drivers.manual.ids': [],
  'drivers.age.badge': ['value'],
  'drivers.filter.Vendor': [],
  'drivers.filter.Managed': [],
};
const placeholders = (text: string) => [...text.matchAll(/\{(\w+)\}/g)].map((m) => m[1]).sort();

test('every new string exists in English and Arabic with the placeholders the page passes', () => {
  for (const [key, args] of Object.entries(KEYS)) {
    assert.ok(hasMessageKey(key), `${key} missing`);
    const en = (enCatalog as Record<string, string>)[key];
    const ar = (arCatalog as Record<string, string>)[key];
    assert.deepEqual(placeholders(en), args, `${key} en`);
    assert.deepEqual(placeholders(ar), args, `${key} ar`);
    assert.match(ar, arabic, `${key} ar text`);
  }
});

test('a registry or ISO driver date parses; anything else is unknown, not zero', async () => {
  const { parseDriverDate } = await import('../src/features/drivers/age.ts');
  assert.equal(parseDriverDate('2023-06-12'), Date.UTC(2023, 5, 12));
  assert.equal(parseDriverDate('6-21-2006'), Date.UTC(2006, 5, 21));
  assert.equal(parseDriverDate('6/21/2006'), Date.UTC(2006, 5, 21));
  assert.equal(parseDriverDate(''), undefined);
  assert.equal(parseDriverDate('not a date'), undefined);
  assert.equal(parseDriverDate('13-40-2006'), undefined);
});

test('only a vendor-installed package (oemNN.inf) is a vendor driver, so Windows in-box drivers get no age', async () => {
  const { isVendorDriver, vendorDriverAgeYears } = await import('../src/features/drivers/age.ts');
  assert.equal(isVendorDriver({ provider: 'Intel', version: '1', infPath: 'oem12.inf', date: '2023-06-12' }), true);
  assert.equal(isVendorDriver({ provider: 'Intel', version: '1', infPath: 'OEM3.INF', date: '2023-06-12' }), true);
  assert.equal(isVendorDriver({ provider: 'Intel(R) PMT', version: '1', infPath: 'intelpmt.inf', date: '6-21-2006' }), false);
  assert.equal(isVendorDriver(null), false);
  const years = vendorDriverAgeYears(withDriver('iCLS', 'oem12.inf', '2023-06-12'), NOW);
  assert.ok(years !== undefined && Math.abs(years - 3.33) < 0.05, `${years}`);
  assert.equal(vendorDriverAgeYears(withDriver('PMT', 'intelpmt.inf', '6-21-2006'), NOW), undefined);
  assert.equal(vendorDriverAgeYears(withDriver('No date', 'oem1.inf', ''), NOW), undefined);
});

test('the Vendor filter lists vendor drivers oldest first, the Managed filter the vendor-managed ones, the rest as before', async () => {
  const { filterDevices } = await import('../src/features/drivers/view.ts');
  const devices = [
    withDriver('Wi-Fi', 'oem4.inf', '2025-12-06'),
    withDriver('PMT inbox', 'intelpmt.inf', '6-21-2006'),
    withDriver('iCLS', 'oem12.inf', '2023-06-12'),
    withDriver('Undated', 'oem9.inf', ''),
    device({ displayName: 'GPU', displayManaged: true, className: 'Display' }),
    device({ displayName: 'Missing', missingDriver: true, hasProblem: true, problemCode: 28 }),
  ];
  assert.deepEqual(filterDevices(devices, 'Vendor', '', NOW).map((d) => d.displayName), ['iCLS', 'Wi-Fi', 'Undated']);
  assert.deepEqual(filterDevices(devices, 'Managed', '', NOW).map((d) => d.displayName), ['GPU']);
  assert.deepEqual(filterDevices(devices, 'Missing', '', NOW).map((d) => d.displayName), ['Missing']);
  assert.equal(filterDevices(devices, 'All', '', NOW).length, 6);
  assert.deepEqual(filterDevices(devices, 'All', 'wi-fi', NOW).map((d) => d.displayName), ['Wi-Fi']);
});

test('the page scans the local inventory when it opens, once, and only when connected and nothing was scanned', async () => {
  const { shouldAutoScan } = await import('../src/features/drivers/view.ts');
  assert.equal(shouldAutoScan('Idle', true, false), true);
  assert.equal(shouldAutoScan('Idle', false, false), false);
  assert.equal(shouldAutoScan('Idle', true, true), false);
  for (const state of ['Ready', 'Failed', 'InventoryScanning', 'UpdateSearching', 'Matching']) assert.equal(shouldAutoScan(state, true, false), false, state);
});

test('a scan that read only the local cache says updates were not searched, with the date Windows last did', async () => {
  const { searchNotice } = await import('../src/features/drivers/view.ts');
  assert.deepEqual(searchNotice({ state: 'Ready', searchScope: 'LocalCacheOnly', windowsLastOnlineSearch: '2026-01-20' }), { date: '2026-01-20' });
  assert.deepEqual(searchNotice({ state: 'Ready', searchScope: 'LocalCacheOnly', windowsLastOnlineSearch: '' }), { date: '' });
  assert.equal(searchNotice({ state: 'Ready', searchScope: 'Online', windowsLastOnlineSearch: '' }), null);
  assert.equal(searchNotice({ state: 'Idle', searchScope: '', windowsLastOnlineSearch: '' }), null);
  assert.equal(searchNotice({ state: 'Matching', searchScope: 'LocalCacheOnly', windowsLastOnlineSearch: '' }), null);
});

test('missing-driver devices with nothing installable are grouped by vendor with their hardware IDs', async () => {
  const { manualRoutingGroups } = await import('../src/features/drivers/routing.ts');
  const pci = (dev: string) => device({ displayName: 'PCI Device', missingDriver: true, hasProblem: true, problemCode: 28, instanceId: `PCI\\VEN_8086&DEV_${dev}\\3`, hardwareIds: [`PCI\\VEN_8086&DEV_${dev}&SUBSYS_88821043&REV_11`, `PCI\\VEN_8086&DEV_${dev}`] });
  const offered = device({ displayName: 'Offered', missingDriver: true, hasProblem: true, problemCode: 28, hardwareIds: ['PCI\\VEN_8086&DEV_1111'], candidates: [{ selectable: true } as never] });
  const groups = manualRoutingGroups([
    pci('7A4C'), pci('7A4D'),
    device({ displayName: 'ACPI', missingDriver: true, hasProblem: true, problemCode: 28, hardwareIds: ['ACPI\\VEN_INTC&DEV_1085', 'ACPI\\INTC1085'] }),
    device({ displayName: 'Realtek', missingDriver: true, hasProblem: true, problemCode: 28, hardwareIds: ['PCI\\VEN_10EC&DEV_8168'] }),
    device({ displayName: 'Mystery', missingDriver: true, hasProblem: true, problemCode: 28, hardwareIds: ['XYZ\\FOO'] }),
    offered,
    device({ displayName: 'Fine', driver: { provider: 'Intel', version: '1', infPath: 'oem1.inf', date: '2025-01-01' } }),
  ]);
  assert.deepEqual(groups.map((g) => [g.vendor, g.devices.length]), [['Intel', 3], ['Realtek', 1], ['', 1]]);
  assert.deepEqual(groups[0].hardwareIds, ['PCI\\VEN_8086&DEV_7A4C', 'PCI\\VEN_8086&DEV_7A4D', 'ACPI\\VEN_INTC&DEV_1085']);
  assert.deepEqual(manualRoutingGroups([]), []);
});

test('the support address is text for a known maker and empty for an unknown one; a placeholder machine name is not shown', async () => {
  const { supportRoot, machineLabel } = await import('../src/features/drivers/routing.ts');
  assert.equal(supportRoot('ASUSTeK COMPUTER INC.'), 'www.asus.com/support');
  assert.equal(supportRoot('Intel'), 'www.intel.com/support');
  assert.equal(supportRoot('Micro-Star International Co., Ltd.'), 'www.msi.com/support');
  assert.equal(supportRoot('Some Unknown Maker'), '');
  assert.equal(supportRoot(''), '');
  assert.doesNotMatch(supportRoot('ASUS'), /^https?:/);
  assert.equal(machineLabel({ machineManufacturer: 'ASUS', machineModel: 'System Product Name', boardProduct: 'PRIME Z790-P' }), 'ASUS PRIME Z790-P');
  assert.equal(machineLabel({ machineManufacturer: 'Dell Inc.', machineModel: 'XPS 9530', boardProduct: '0K1B2C' }), 'Dell Inc. XPS 9530');
  assert.equal(machineLabel({ machineManufacturer: 'System manufacturer', machineModel: 'System Product Name', boardProduct: 'To Be Filled By O.E.M.' }), '');
  assert.equal(machineLabel({ machineManufacturer: '', machineModel: '', boardProduct: '' }), '');
});
