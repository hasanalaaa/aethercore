# Gate 4 — candidate devices, measured 2026-09-12

Gate 4 installs a driver, verifies it, rolls it back, and verifies the rollback.
It needs a device whose driver can be *changed* — a device on an inbox Microsoft
driver has no second version to install, so it cannot exercise the gate at all.

**The owner picks one row from §1.** Nothing here has been installed. No driver
has been touched.

## Method

```
Get-PnpDevice                      # every device and its class
Get-CimInstance Win32_PnPSignedDriver | ? InfName -match '^oem'   # present devices on third-party drivers
pnputil /enum-drivers              # every package in the driver store, with versions
```

The distinction that matters: `pnputil /enum-drivers` lists packages that are
**staged in the driver store**, which is not the same as a device being
**attached**. Three otherwise-ideal candidates fail on exactly that — see §3.

## Allowed and forbidden classes

Allowed: printer-class, HID-class, USB-peripheral.

Forbidden, without exception: storage, chipset, GPU, network, or anything in the
boot path.

**The Intel Arc display driver (31.0.101.5007, 2023-11-18) is present on this
machine and is NOT a candidate.** A display driver is precisely the class that
must not be the safe test device: a bad one takes the console with it, which is
the failure mode Gate 4's recovery posture exists to survive. The same exclusion
covers `oem91.inf` (NVIDIA RTX 4060, 32.0.16.1656) and `oem43.inf` (Intel RST
VMD storage controller) — the latter is in the boot path.

---

## §1 Candidates

### A — Intel(R) HID Event Filter · `oem92.inf` · **the only in-class candidate attached today**

| | |
|---|---|
| device | Intel(R) HID Event Filter (`ACPI\INTC1070\2&DABA3FF&0`) |
| class | **HIDClass** — allowed |
| current driver | `hideventfilter.inf`, Intel(R) Corporation, **2.2.2.5**, dated **2023-04-19** |
| other versions in the driver store | **none** — one package only |
| obtainable elsewhere | **not verified.** Intel and MSI publish HID Event Filter packages for this platform (MSI MS-15P3), but no alternate version has been downloaded or hash-checked, and Windows Update offers this machine no drivers |
| rollback restores | 2.2.2.5, via `pnputil /restore-driver` or Device Manager's Roll Back Driver, which is available only *after* a different version is installed over it |
| **risk** | **Low–moderate.** This driver services hotkey and chassis-button events (volume keys, the power button's HID path). A bad version degrades those keys; it does not affect the PS/2 keyboard, the HID touchpad, storage, display or boot — each of those is a separate device on a separate inbox driver. The machine remains usable and loggable-into if it fails. |
| **the honest caveat** | with only one package in the store there is no locally-staged version to install, so this candidate cannot run until a second version is obtained and verified. That is owner action 2b below. |

### B — Canon G3010 series printer · `oem72.inf` · **best candidate, but the device is not attached**

| | |
|---|---|
| device | not present. `oem72.inf` (`g3010p6.inf`, Canon, 3.20.2.40, 2023-06-16) is staged in the driver store from a previous attachment; `oem65.inf` (`g3010sc.inf`, scanner, 20.10.0.4) likewise |
| class | **Printer** — the doc's first-named allowed class |
| **risk** | **Lowest of any option.** A printer is not in any path the OS needs. A failed printer driver loses printing and nothing else |
| **what it needs** | the owner plugs the printer in. Canon publishes multiple public driver versions for the G3010 series, so both an install target and a rollback target are genuinely obtainable |
| **recommendation** | **If the owner can attach this printer, it is the right device and A should not be used.** It is the only option where the blast radius of a failure is confined to a peripheral the machine never depends on |

### C — Logitech LampArray · `oem40.inf` · **not attached**

| | |
|---|---|
| device | not present. `oem40.inf` (`logi_lamparray_usb.inf`, Logitech, 1.1.84.7627, 2026-02-27) staged only |
| class | **USB** — allowed |
| **risk** | **Low.** An RGB-lighting peripheral; no OS dependency |
| **what it needs** | the owner attaches the Logitech device. Viable if the printer is not available |

---

## §2 Considered and rejected

| device | why not |
|---|---|
| Intel(R) Arc(TM) Graphics `oem80.inf` 31.0.101.5007 | **display** — named in the brief as explicitly not a candidate, and correctly so |
| NVIDIA GeForce RTX 4060 `oem91.inf` 32.0.16.1656 | **display** |
| Intel RST VMD Controller `oem43.inf` | **storage, boot path** |
| Intel Wi-Fi 6E AX211 `oem15.inf`, Realtek PCIe GbE `oem34.inf` | **network** |
| Intel Management Engine, Serial IO, SMBus, SPI, Shared SRAM (`oem32`, `oem21`, `oem29`, `oem42`) | **chipset** |
| MS-15P3 SBIOS `oem56.inf` | **firmware** — worst possible choice |
| NVIDIA High Definition Audio `oem100.inf` 1.4.6.3 | **MEDIA class, outside the allowed set.** Tempting because the store already holds three versions (1.4.3.2, 1.4.5.0, 1.4.6.3), so install *and* rollback targets exist offline with no download — but the brief restricts the classes, and audio is not one of them. Recorded here because if the owner ever widens the allowed classes, this is the technically easiest device on the machine |
| Realtek High Definition Audio `oem57.inf` 6.0.9635.1 | **MEDIA class** |
| HD Webcam, USB Input Device, HID-compliant device, Microsoft Print to PDF | in allowed classes, but all four are on **inbox Microsoft drivers** (`usbvideo.inf`, `input.inf`, `printqueue.inf`, all reporting 10.0.26100.9444 / the 2006-06-21 inbox date). Microsoft ships one inbox driver per OS build; there is no second version to install, so none of them can exercise an install-and-rollback gate |

---

## §3 The finding that shapes this list

Of the three devices whose class and driver provenance make them ideal Gate 4
candidates — the Canon printer, the Canon scanner, the Logitech LampArray —
**none is attached.** Their driver packages survive in the store from earlier
attachments, which is why a naive `pnputil /enum-drivers` reading suggests four
or five good candidates where the machine actually offers one.

The single attached device that is both in an allowed class and on a
third-party driver is **A, the Intel HID Event Filter** — and even that has only
one version staged, so it needs a download before it can be used.

This is why the brief's "Windows Update offers this machine zero drivers, so
there is no candidate yet" is right about the conclusion. It is not that the
machine has no candidate devices; it is that the candidates are unplugged.

---

## §4 What the owner does

1. **Preferred:** attach the Canon G3010 printer, then choose **B**. Lowest
   risk, and both driver versions are publicly obtainable from Canon.
2. **Otherwise:** attach the Logitech device and choose **C**.
3. **Fallback:** choose **A**, and additionally obtain a second Intel HID Event
   Filter version from Intel or MSI's MS-15P3 support page. The runbook hashes
   and records whatever package is supplied before installing it.

Whichever is chosen, Gate 4 does not start until the recovery media has been
boot-tested — see `../LEDGER.md` §3, owner action 1.
