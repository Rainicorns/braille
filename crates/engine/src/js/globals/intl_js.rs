use rquickjs::Ctx;

pub(super) fn register_intl_js(ctx: &Ctx<'_>) {
    // Intl object with constructors backed by native __n_intlFormatDate/__n_intlFormatNumber
    ctx.eval::<(), _>(r#"
        globalThis.Intl = {
            DateTimeFormat: function DateTimeFormat(locales, opts) {
                if (!(this instanceof DateTimeFormat)) return new DateTimeFormat(locales, opts);
                this._opts = opts || {};
            },
            NumberFormat: function NumberFormat(locales, opts) {
                if (!(this instanceof NumberFormat)) return new NumberFormat(locales, opts);
                this._opts = opts || {};
            },
            Collator: function Collator(locales, opts) {
                if (!(this instanceof Collator)) return new Collator(locales, opts);
                this._opts = opts || {};
            },
            PluralRules: function PluralRules(locales, opts) {
                if (!(this instanceof PluralRules)) return new PluralRules(locales, opts);
                this._opts = opts || {};
            },
            RelativeTimeFormat: function RelativeTimeFormat(locales, opts) {
                if (!(this instanceof RelativeTimeFormat)) return new RelativeTimeFormat(locales, opts);
                this._opts = opts || {};
            },
            getCanonicalLocales: function(locales) { return ['en-US']; },
        };

        Intl.DateTimeFormat.prototype.format = function(date) {
            var ts = (date instanceof Date) ? date.getTime() : Number(date);
            return __n_intlFormatDate(ts, JSON.stringify(this._opts));
        };
        Intl.DateTimeFormat.prototype.resolvedOptions = function() {
            var r = { locale: 'en-US', calendar: 'gregory', numberingSystem: 'latn', timeZone: 'UTC' };
            var o = this._opts;
            if (o.year) r.year = o.year;
            if (o.month) r.month = o.month;
            if (o.day) r.day = o.day;
            if (o.hour) r.hour = o.hour;
            if (o.minute) r.minute = o.minute;
            if (o.second) r.second = o.second;
            if (o.weekday) r.weekday = o.weekday;
            return r;
        };
        Intl.DateTimeFormat.supportedLocalesOf = function() { return ['en-US']; };

        Intl.NumberFormat.prototype.format = function(n) {
            return __n_intlFormatNumber(Number(n), JSON.stringify(this._opts));
        };
        Intl.NumberFormat.prototype.resolvedOptions = function() {
            return { locale: 'en-US', numberingSystem: 'latn', style: this._opts.style || 'decimal', minimumFractionDigits: 0, maximumFractionDigits: 3 };
        };
        Intl.NumberFormat.supportedLocalesOf = function() { return ['en-US']; };

        Intl.Collator.prototype.compare = function(a, b) {
            a = String(a); b = String(b);
            if (a < b) return -1;
            if (a > b) return 1;
            return 0;
        };
        Intl.Collator.prototype.resolvedOptions = function() {
            return { locale: 'en-US', usage: 'sort', sensitivity: 'variant', collation: 'default' };
        };
        Intl.Collator.supportedLocalesOf = function() { return ['en-US']; };

        Intl.PluralRules.prototype.select = function(n) {
            return n === 1 ? 'one' : 'other';
        };
        Intl.PluralRules.prototype.resolvedOptions = function() {
            return { locale: 'en-US', type: 'cardinal', pluralCategories: ['one', 'other'] };
        };
        Intl.PluralRules.supportedLocalesOf = function() { return ['en-US']; };

        Intl.RelativeTimeFormat.prototype.format = function(value, unit) {
            var v = Math.abs(value);
            var u = String(unit).replace(/s$/, '');
            var label = v === 1 ? u : u + 's';
            if (value < 0) return v + ' ' + label + ' ago';
            if (value > 0) return 'in ' + v + ' ' + label;
            return 'now';
        };
        Intl.RelativeTimeFormat.prototype.resolvedOptions = function() {
            return { locale: 'en-US', style: 'long', numeric: 'always' };
        };
        Intl.RelativeTimeFormat.supportedLocalesOf = function() { return ['en-US']; };
    "#).unwrap();
}
