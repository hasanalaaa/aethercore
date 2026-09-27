// Lets `node --experimental-strip-types --test` load the app's own modules, which import each
// other without the `.ts` extension that Vite resolves and Node does not.
import { registerHooks } from 'node:module';
registerHooks({
  resolve(specifier, context, next) {
    if (specifier.startsWith('.') && !/\.[cm]?[jt]s$/.test(specifier)) {
      try { return next(`${specifier}.ts`, context); } catch { /* not a .ts module */ }
    }
    return next(specifier, context);
  },
});
