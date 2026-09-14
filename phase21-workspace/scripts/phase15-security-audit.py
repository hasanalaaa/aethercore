#!/usr/bin/env python3
from pathlib import Path
import argparse,json,re,sys
ROOT=Path(__file__).resolve().parents[1]
PARSER=argparse.ArgumentParser();PARSER.add_argument("--output",type=Path);ARGS=PARSER.parse_args()
checks={}
# P59 / DBT-P58-005: one shared reader that raises instead of substituting "".
# P58 fixed where this reader looked - `.github/` resolves against the
# repository root - but not what it did when the look failed, so a check
# asserting something is ABSENT still passed against a file never opened.
# No bytecode: `omega-evidence.py` runs each gate against a disposable clone
# and treats ANY new file in it as a source mutation, so a `__pycache__`
# entry for this import would be reported as the gate rewriting the tree.
sys.dont_write_bytecode = True
sys.path.insert(0, str(Path(__file__).resolve().parent))
from gate_reader import SourceReader, contains, count, position  # noqa: E402

_READER = SourceReader(ROOT)
read = _READER.read
# `DBT-P63-004`: the router is a module tree - `router.rs` plus `router/*.rs`.
# What these checks assert is its verbs, so they read the tree, not the root.
read_module = _READER.read_module
def check(name,ok,**detail): checks[name]={'ok':bool(ok),**detail}
# `contains`, not `in`: whitespace-insensitive, defined once. `DBT-P61-001`.
def has(text,*tokens): return all(contains(text,t) for t in tokens)

root_cargo=read('Cargo.toml'); update_cargo=read('crates/update-engine/Cargo.toml'); download_cargo=read('crates/update-download/Cargo.toml'); desktop_cargo=read('apps/desktop/Cargo.toml')
manifest=read('crates/update-engine/src/manifest.rs'); coordinator=read('crates/update-engine/src/coordinator.rs'); platform=read('crates/update-engine/src/platform.rs'); downloader=read('crates/update-download/src/lib.rs')
router=read_module('services/maintenance-service/src/router.rs'); composition=read('services/maintenance-service/src/composition.rs'); service_cargo=read('services/maintenance-service/Cargo.toml'); protocol=read('services/maintenance-service/src/protocol.rs'); errors=read('services/maintenance-service/src/errors.rs')
desktop=read('apps/desktop/src/main.rs'); broker=read('apps/update-broker/src/main.rs'); security=read('crates/security/src/lib.rs'); mutation=read('crates/operation-kernel/src/mutation.rs'); foundation=read('crates/windows-foundation/src/lib.rs')
update_proto=read('crates/contracts/proto/update.proto'); support_proto=read('crates/contracts/proto/support_bundle.proto'); operations=read('crates/contracts/proto/operations.proto'); events=read('crates/contracts/proto/events.proto')
support=read('crates/support-bundle/src/lib.rs'); support_service=read('services/maintenance-service/src/support.rs'); support_tool=read('tools/support-bundle-verify/src/main.rs')
persist=read('crates/persistence/src/lib.rs'); migration=read('crates/persistence/migrations/0009_phase15_update.sql')
wix=read('installer/wix/Product.wxs'); build_release=read('scripts/build-release.ps1'); build_installer=read('scripts/build-installer.ps1'); trust_validate=read('scripts/validate-update-trust.ps1'); trust_gen=read('scripts/generate-update-trust.ps1'); manifest_build=read('scripts/build-update-manifest.ps1'); trust_template=read('release/update-trust.template.json')
ui=read('apps/ui/src/features/system-care/SystemCarePanel.svelte'); ui_controller=read('apps/ui/src/features/system-care/controller.ts'); contracts=read('apps/ui/src/lib/contracts.ts'); stream=read('apps/ui/src/platform/stream-state.ts')
ci=read('.github/workflows/ci.yml'); release=read('.github/workflows/release.yml'); verify=read('scripts/verify-phase15.ps1'); crypto_ps=read('scripts/phase15-crypto-tests.ps1')
docs=read('docs/SECURE_UPDATE_AND_SUPPORT_EXPORT.md')+'\n'+read('docs/adr/0017-secure-update-and-cryptographic-support-export.md')

required=[
 'crates/update-engine/Cargo.toml','crates/update-engine/src/manifest.rs','crates/update-engine/src/coordinator.rs','crates/update-engine/src/platform.rs',
 'crates/update-download/Cargo.toml','crates/update-download/src/lib.rs','crates/support-bundle/Cargo.toml','crates/support-bundle/src/lib.rs',
 'apps/update-broker/Cargo.toml','apps/update-broker/src/main.rs','tools/update-manifest/Cargo.toml','tools/update-manifest/src/main.rs','tools/support-bundle-verify/Cargo.toml','tools/support-bundle-verify/src/main.rs',
 'crates/contracts/proto/update.proto','crates/contracts/proto/support_bundle.proto','crates/persistence/migrations/0009_phase15_update.sql',
 'release/update-trust.template.json','scripts/generate-update-trust.ps1','scripts/validate-update-trust.ps1','scripts/build-update-manifest.ps1',
 'scripts/phase15-security-audit.py','scripts/phase15-security-audit.ps1','scripts/phase15-crypto-tests.ps1','scripts/verify-phase15.ps1',
 'docs/SECURE_UPDATE_AND_SUPPORT_EXPORT.md','docs/adr/0017-secure-update-and-cryptographic-support-export.md','PHASE_15_DELIVERABLES.md','PHASE_15_VALIDATION_SUMMARY.md'
]
check('required_artifacts',all((ROOT/p).is_file() for p in required),missing=[p for p in required if not (ROOT/p).is_file()])

# Network authority and static manifest trust.
check('desktop_only_http_crate','"crates/update-download"' in root_cargo and 'aethercore-update-download' in desktop_cargo)
check('service_update_engine_has_no_reqwest','reqwest' not in update_cargo.lower() and 'reqwest' not in service_cargo.lower() and 'reqwest' not in platform.lower())
check('downloader_https_only',has(downloader,'url.starts_with("https://")','authority.contains(\'@\')','url.contains(\'#\')'))
check('downloader_redirects_disabled','redirect(reqwest::redirect::Policy::none())' in downloader)
check('downloader_finite_timeouts',has(downloader,'connect_timeout(Duration::from_secs(10))','timeout(Duration::from_secs(60))'))
check('downloader_body_bounded',has(downloader,'take(max_bytes as u64+1)','ResponseTooLarge'))
check('downloader_size_bounded',has(downloader,'content_length()','expected_size','SizeMismatch'))
check('downloader_temp_cleanup',has(downloader,'TempDownload','impl Drop for TempDownload','remove_file(&self.path)'))
check('downloader_hashes_stream',has(downloader,'Sha256::new()','hash.update(&buffer[..read])','hex::encode(hash.finalize())'))
check('manifest_exact_bytes_signed_before_parse',manifest.find('verifying.verify(manifest_bytes') < manifest.find('let manifest: UpdateManifest = serde_json::from_slice(manifest_bytes)'))
check('manifest_ed25519_and_key_id',has(manifest,'VerifyingKey::from_bytes','signature.key_id != trust.key_id','Signature::from_bytes'))
check('manifest_channel_binding',has(manifest,'trust.channel != expected_channel','value.channel != expected_channel'))
check('manifest_expiry_and_future_bounds',has(manifest,'MAX_MANIFEST_LIFETIME_MS','ManifestError::Expired','ManifestError::FutureDated'))
check('manifest_https_package_only',has(manifest,'UpdatePackageKind::Burn','validate_https_url(&release.package.url)'))
check('manifest_hash_and_size_bounds',has(manifest,'MAX_PACKAGE_BYTES','sha256.len() != 64','size_bytes == 0'))
check('trust_duplicate_channel_rejected',has(manifest,'seen_channels','!seen_channels.insert'))
check('disabled_template_has_no_fake_key',json.loads(trust_template).get('enabled') is False and json.loads(trust_template).get('channels')==[])
check('trust_generator_https_and_key_bounds',has(trust_gen,"$uri.Scheme -ne 'https'",'^[0-9a-fA-F]{64}$','Duplicate update channel'))
check('trust_validator_requires_stable_for_enabled',has(trust_validate,"if(-not $seen['stable'])",'RequireEnabled','All-zero update public key is forbidden'))

# Service descriptor / upload model: no privileged downloader.
# rustfmt turned both arms into braced blocks. The arm, the error and the string
# are unchanged; the `=>Err(` spelling is not. `DBT-P61-001`.
check('legacy_service_download_rpc_disabled',has(router,'request::Payload::CheckForUpdates(_) => { Err("legacy service-side update download is disabled"','request::Payload::StageUpdate(_) => { Err("legacy service-side update download is disabled"'))
check('descriptor_submit_upload_rpc_present',has(router,'GetUpdateCheckDescriptor','SubmitUpdateManifest','BeginUpdateStageUpload','WriteUpdateStageChunk','FinalizeUpdateStageUpload','CancelUpdateStageUpload'))
check('desktop_fetches_service_descriptors',has(desktop,'GetUpdateCheckDescriptor','UpdateCheckDescriptor','BeginUpdateStageUpload','UpdateStageUploadDescriptor'))
check('desktop_network_url_not_ui_parameter',all(token not in ui_controller for token in ['manifestUrl','signatureUrl','packageUrl','downloadUrl']) and has(desktop,'&descriptor.manifest_url','&descriptor.signature_url','&descriptor.package_url'))
check('desktop_user_scope_update_cache',has(desktop,'LOCALAPPDATA','AetherCore','UpdateCache') and 'staged_path' not in ui_controller)
check('desktop_bounded_chunk_upload',has(desktop,'MAX_STAGE_CHUNK_BYTES','WriteUpdateStageChunk','offset') and has(coordinator,'MAX_STAGE_CHUNK_BYTES:u32=256*1024'))
check('service_upload_exact_offset',has(coordinator,'upload.offset!=offset','meta.len()!=upload.offset','checked_add(data.len() as u64)'))
check('service_upload_exact_size',has(coordinator,'upload.offset!=upload.release.package.size_bytes','SizeMismatch'))
check('service_staging_path_derived',has(coordinator,'expected_staged_path','owner_scope','sha256.to_ascii_lowercase()','AetherCoreUpdate-{owner_scope}-{release_id}-{}.exe') and 'staged_path' not in update_proto.split('message BeginUpdateStageUploadRequest',1)[1].split('}',1)[0])
check('service_revalidates_hash_authenticode_hash',count(coordinator,'verify_file_hash_size(&upload.temp_path')>=2 and has(coordinator,'self.verifier.verify_authenticode(&upload.temp_path)'))
check('intent_revalidates_hash_authenticode_hash',count(coordinator,'verify_file_hash_size(&path,&release.sha256,release.size_bytes)')>=2 and has(coordinator,'self.verifier.verify_authenticode(&path)'))
check('claim_revalidates_artifact',has(coordinator,'verify_file_hash_size(&path,&intent.release.sha256,intent.release.size_bytes)','self.verifier.verify_authenticode(&path)'))
check('update_mutation_workload_reserved','Update' in mutation and 'MutationWorkload::Update' in coordinator)
check('one_shot_intent_claim',has(coordinator,'claimed:bool','record.claimed','record.claimed=true','!v.claimed'))
claim_start=coordinator.find('pub fn claim_install')
claim_end=coordinator.find('pub fn complete_install',claim_start)
claim_body=coordinator[claim_start:claim_end] if claim_start>=0 and claim_end>claim_start else ''
claim_reserved=position(claim_body,'record.claimed=true')
claim_verify=position(claim_body,'verify_file_hash_size(&path')
claim_lease=position(claim_body,'self.mutations.try_acquire(MutationWorkload::Update')
check('claim_reservation_linearizes_before_verification_and_lease',claim_reserved>=0 and claim_verify>claim_reserved and claim_lease>claim_verify and has(claim_body,'if result.is_err()','record.claimed=false'))
check('claim_cleanup_preserves_inflight_reservation',has(coordinator,'intents.retain(|_,v|v.claimed||v.intent.expires_unix_ms>=now)','self.intents.lock().unwrap_or_else(|p|p.into_inner()).remove(intent_id)','cleanup_does_not_erase_an_inflight_claim_reservation'))
check('declined_or_cancelled_intent_restores_staged_state',has(coordinator,'cancel_install_intent','UpdateState::AwaitingConsent','UpdateState::Staged') and has(desktop,'cancel_update_install_intent(&intent_id)') and has(broker,'cancel_intent(&intent_id)'))
check('expired_intent_does_not_stick_awaiting_consent',has(coordinator,'expired_intent_owners','s.state==UpdateState::AwaitingConsent','s.staged_release.is_some()'))
check('durable_execution_guard',has(migration,'active_update_execution','slot = 1','expected_sha256','staged_path') and has(coordinator,'replace_update_execution_guard','recover_execution_guard','clear_update_execution_guard'))
check('execution_guard_no_url_or_command',all(token not in migration.lower() for token in ['package_url','manifest_url','command_line','arguments','consent_secret']))
check('manifest_floor_equivocation_hash',has(migration,'manifest_sha256','update_manifest_floor') and has(coordinator,'manifest.sequence==floor.highest_sequence','manifest_sha256.eq_ignore_ascii_case'))
check('minimum_windows_build_enforced',has(platform,'RtlGetVersion','dwOSVersionInfoSize' if False else 'size:std::mem::size_of::<RtlOsVersionInfoW>()','info.build') and has(coordinator,'release.minimum_windows_build<=self.current_windows_build','releases_requiring_newer_windows_build_are_not_offered'))
check('stage_failures_leave_recoverable_state',has(coordinator,'mark_stage_failed','update.status.stageFailed','assert_eq!(f.coordinator.snapshot(&f.owner).state,UpdateState::Failed)'))
check('install_intent_claim_linearized',has(coordinator,'record.claimed=true','record.claimed=false','claim_reserves_intent_before_machine_lease_and_releases_reservation_on_busy'))

# Elevated fixed-purpose broker.
check('update_broker_exact_cli_surface',has(broker,'if args.len()!=5','--intent-id','--locale') and all(token not in broker for token in ['--url','--path','--command','--args']))
check('service_requires_exact_elevated_update_broker',has(router,'require_update_broker(peer)?','expected_update_broker_path','is_expected_broker(peer,&expected)') and has(security,'peer.elevated','image_path'))
check('broker_never_downloads',all(token not in broker.lower() for token in ['reqwest','winhttp','http://','https://']))
check('broker_hash_authenticode_hash',count(broker,'verify_file_hash_size(&path')>=2 and has(broker,'verify_authenticode(&path)'))
check('broker_runs_exact_ticket_path_without_args',has(broker,'Command::new(&path).status()','let path=std::path::PathBuf::from(&ticket.staged_path)') and '.arg(' not in broker and '.args(' not in broker)
check('broker_only_accepts_success_or_reboot',has(broker,'matches!(exit_code,0|3010)','complete_with_retry'))
check('broker_completion_retry_bounded',has(broker,'Duration::from_secs(90)','Instant::now()<deadline'))

# Authenticode runtime policy and release authority.
check('winverifytrust_chain_validation',has(platform,'WinVerifyTrust','WINTRUST_ACTION_GENERIC_VERIFY_V2','WTD_REVOKE_WHOLECHAIN','WTD_REVOCATION_CHECK_CHAIN'))
check('wix_installs_update_broker_and_trust',has(wix,'UpdateBrokerExe','UpdateTrustJson','aethercore-update-broker.exe','update-trust.json'))
check('release_builds_and_signs_update_broker',has(build_release,'aethercore-update-broker','sign-artifacts.ps1'))
check('release_validates_update_trust',has(build_release,'validate-update-trust.ps1','Signed Phase 15 release packaging requires -UpdateTrustPath'))
check('installer_requires_update_payloads',has(build_installer,"'aethercore-update-broker.exe'","'update-trust.json'"))
check('manifest_build_uses_final_burn_metadata',has(manifest_build,"kind='burn'",'Get-FileHash $bundle -Algorithm SHA256','$size=(Get-Item $bundle).Length'))
check('manifest_sign_tool_package_name_correct','-p aethercore-update-manifest-tool' in manifest_build)
check('manifest_signing_key_external',has(manifest_build,'PrivateKeyPath','Resolve-Path $PrivateKeyPath') and 'private' not in trust_template.lower())

# Privacy-first support bundle.
check('support_allowlist_sections',all(name in support_service for name in ['product.json','diagnostics.json','operation-history.json','scheduler-activity.json']))
check('support_no_raw_dump_or_eventlog_export',all(token not in support_service for token in ['read_to_end','MEMORY.DMP','Minidump','EvtRender','EventLog.xml']))
# rustfmt wrapped `prepare` and added the trailing comma. `DBT-P61-001`.
check('support_preview_precedes_prepare',has(support,'create_preview','pub fn prepare(&self, owner: &str, preview_id: &str,)','PreviewUnavailable'))
check('support_preview_has_privacy_report',has(support,'PrivacyReport','user_path_redactions','account_identifier_redactions','hardware_serial_redactions','email_redactions'))
check('support_user_path_and_account_redaction',has(support,'<redacted-account>','%USERPROFILE%','<redacted-sid>','<redacted-email>'))
check('support_serial_non_linkable_redaction',has(support,'<redacted-hardware-serial>','serial_redaction_is_non_linkable') and 'serial-hash' not in support)
check('support_section_and_archive_caps',has(support,'MAX_SECTION_BYTES','MAX_BUNDLE_BYTES','MAX_CHUNK_BYTES'))
check('support_deterministic_sorted_payloads',has(support,'files.sort_by','BTreeMap','canonical_value'))
check('support_per_file_sha_manifest',has(support,'ManifestFile','size_bytes','sha256','payload_root_sha256'))
check('support_installation_claim_not_vendor_attestation',has(support,'installation-ed25519','not a vendor or hardware attestation'))
# rustfmt wrapped `verify_archive` and added the trailing comma. `DBT-P61-001`.
check('support_strong_verifier_requires_independent_fingerprint',has(support,'pub fn verify_archive(bytes: &[u8], expected_public_key_fingerprint_sha256: &str,)','actual_fingerprint.eq_ignore_ascii_case(expected_public_key_fingerprint_sha256)','embedded_key_is_not_a_root_of_trust'))
check('support_rejects_unmanifested_entries',has(support,'unmanifested archive entry','unmanifested_entry_is_rejected'))
check('support_recomputes_root_hash',has(support,'payload_root_sha256','HashMismatch'))
# Not whitespace: `return Err(...)` is a statement and carries its own semicolon,
# which the token omitted. `DBT-P61-001`.
check('support_owner_scoped_chunks',has(support,'if record.owner != owner { return Err(SupportBundleError::Ownership); }','offset > total'))
check('support_service_never_accepts_destination_path',all(token not in support_proto for token in ['destination_path','output_path','folder_path']) and all(token not in router for token in ['destination_path','output_path']))
check('support_desktop_writes_user_downloads',has(desktop,'USERPROFILE','Downloads','ReadSupportBundleChunk'))
check('support_desktop_never_overwrites_existing',has(desktop,'non_overwriting_support_path','create_new(true)','.aetherdiag-new'))
check('support_desktop_checks_public_key_fingerprint',has(desktop,'decode_hex_32','computed_fingerprint','public_key_fingerprint_sha256'))
check('support_export_rehashes_final_bytes',has(desktop,'Sha256::new()','ready.sha256','support.error.integrity'))
check('support_verifier_cli_requires_expected_fingerprint',has(support_tool,'expected-public-key-fingerprint-sha256','verify_archive(&bytes,&fingerprint)'))

# Typed IPC/event/UI.
check('update_proto_has_descriptor_upload_contracts',has(update_proto,'UpdateCheckDescriptorResponse','SubmitUpdateManifestRequest','UpdateStageUploadDescriptorResponse','WriteUpdateStageChunkRequest'))
check('legacy_update_download_fields_marked_deprecated','check_for_updates = 46 [deprecated = true]' in operations and 'stage_update = 48 [deprecated = true]' in operations)
check('support_proto_has_preview_proof_fingerprint',has(support_proto,'SupportBundlePreview','SupportPrivacyReport','public_key_fingerprint_sha256'))
check('typed_update_support_events',has(events,'EVENT_KIND_UPDATE = 19','EVENT_KIND_SUPPORT_BUNDLE = 20','UpdateSnapshot update_snapshot = 28','SupportBundleEvent support_bundle = 29'))
check('desktop_normalizes_update_support_events',has(desktop,'Payload::UpdateSnapshot','Payload::SupportBundle'))
check('renderer_stream_tracks_update_support',has(stream,'updateSnapshot','supportBundleEvent'))
check('ui_update_is_check_stage_install',has(ui,'update.check','update.download','update.install') and has(ui_controller,'checkUpdates','stageLatestUpdate','installStagedUpdate'))
check('ui_support_preview_before_export',has(ui_controller,'previewSupportBundle','supportPreview','if (!preview) return','exportSupportBundle'))
check('ui_shows_privacy_preview_and_verification_fingerprint',has(ui,'support.redactions','support.estimatedSize','support.verificationFingerprint','TechnicalText'))
check('desktop_reverifies_support_proof_before_export',has(desktop,'aethercore_support_bundle::verify_archive(&archive_bytes','support.error.integrity') and position(desktop,'verify_archive(&archive_bytes') < position(desktop,'std::fs::rename(&temporary,&path)'))
check('typed_update_support_error_keys',has(errors,'update.error.integrity','update.error.trust','support.error.integrity','support.error.unavailable'))

# Tests and release gates.
check('update_engine_regressions',has(coordinator,'signed_manifest_floor_rejects_rollback_and_same_sequence_equivocation','stage_upload_is_offset_bounded_and_rejects_truncation','staged_path_is_service_derived_and_update_lease_is_machine_exclusive'))
check('support_tamper_regressions',has(support,'deterministic_archive_and_tamper_detection','proof_claim_is_not_malleable','embedded_key_is_not_a_root_of_trust','unmanifested_entry_is_rejected'))
check('downloader_https_regression','downloader_rejects_non_https_and_ambiguous_authorities' in downloader)
check('phase15_crypto_test_gate',has(crypto_ps,'aethercore-update-engine','aethercore-update-download','aethercore-support-bundle','aethercore-support-bundle-verify'))
check('phase15_verify_inherits_phase14',has(verify,'verify-phase14.ps1','phase15-security-audit.ps1','phase15-crypto-tests.ps1','cargo check --workspace --locked'))
check('phase15_packaging_after_security_gates',verify.find('phase15-security-audit.ps1') < verify.find('build-release.ps1') if 'build-release.ps1' in verify else False)
check('phase15_signed_release_requires_update_trust',has(verify,'AETHERCORE_UPDATE_TRUST_PATH','UpdateTrustPath','RequireSigning'))
check('phase15_ci_release_gate',any(g in ci for g in ['verify-phase15.ps1 -SkipOnlineSupplyChain','verify-phase16.ps1 -SkipOnlineSupplyChain','verify-enterprise.ps1 -SkipOnlineSupplyChain']) and any(g in release for g in ['verify-phase15.ps1 -ReleasePackaging -RequireSigning','verify-phase16.ps1 -ReleasePackaging -RequireSigning','verify-enterprise.ps1 -ReleasePackaging -RequireSigning']))
check('phase15_docs_capture_trust_boundaries',has(docs,'user-scope downloader','MutationSupervisor::Update','installation-local Ed25519','independent fingerprint','WiX') )
check('broker_protected_machine_mutation_lock',has(broker,'UpdateMutationGuard::acquire','MachineMutationGuard::try_acquire()') and has(foundation,'pub struct MachineMutationGuard','SHGetKnownFolderPath','FOLDERID_ProgramData','machine-mutation.lock','OPEN_EXISTING','LockFileEx','UnlockFileEx') and broker.find('UpdateMutationGuard::acquire') < broker.find('claim(&intent_id)'))
check('execution_expiry_is_fail_closed_on_ledger_failure',has(coordinator,'if self.db.clear_update_execution_guard(&ticket_id).is_ok()','self.active.lock().unwrap_or_else(|p|p.into_inner()).take()') and coordinator.find('clear_update_execution_guard(&ticket_id).is_ok()') < coordinator.find('take();',coordinator.find('pub fn reap_expired_execution')))
check('restart_recovery_preserves_exact_release_identity',has(migration,'release_version TEXT NOT NULL','channel TEXT NOT NULL','notes_message_key TEXT NOT NULL','minimum_windows_build INTEGER NOT NULL') and has(coordinator,'record.release_version.clone()','record.notes_message_key.clone()','record.minimum_windows_build','snapshot.state=UpdateState::Installing','durable_execution_recovery_restores_installing_snapshot_and_release_identity'))
check('support_ed25519_strict_verification','verify_strict(manifest_bytes' in support)
check('support_embedded_identifier_redaction_regression',has(support,'embedded_sid_and_email_are_redacted_inside_free_form_text','<redacted-sid>','<redacted-email>'))
check('support_retained_object_quotas',has(support,'MAX_ACTIVE_PREVIEWS_TOTAL','MAX_ACTIVE_BUNDLES_TOTAL','SupportBundleError::ResourceLimit','one_active_bundle_per_owner_is_enforced_and_discard_releases_quota'))
check('support_failure_discards_service_bundle',desktop.count('request(request::Payload::DiscardSupportBundle')>=2 and has(desktop,'if result.is_err()','remove_file(&temporary)'))

ok=all(v['ok'] for v in checks.values());failed=[k for k,v in checks.items() if not v['ok']]
report={'phase':15,'ok':ok,'check_count':len(checks),'checks':checks}
if ARGS.output:
    ARGS.output.parent.mkdir(parents=True,exist_ok=True)
    ARGS.output.write_text(json.dumps(report,indent=2,sort_keys=True)+'\n',encoding='utf-8')
print(json.dumps({'ok':ok,'checks':len(checks),'failed':failed},indent=2));sys.exit(0 if ok else 1)
