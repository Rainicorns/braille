// airbnb_extract.js — Reusable extraction for Airbnb search results
// Returns JSON string with all listings + deep links
(function() {
    var el = document.getElementById('data-deferred-state-0');
    if (!el) return JSON.stringify({error: 'no deferred state found', listings: []});

    var raw;
    try { raw = JSON.parse(el.textContent); }
    catch(e) { return JSON.stringify({error: 'parse failed: ' + e.message, listings: []}); }

    var niobe = raw.niobeClientData;
    if (!niobe || !niobe[0] || !niobe[0][1]) return JSON.stringify({error: 'no niobe data', listings: []});

    var search = niobe[0][1].data;
    if (!search || !search.presentation || !search.presentation.staysSearch)
        return JSON.stringify({error: 'no staysSearch', listings: []});

    var results = search.presentation.staysSearch.results.searchResults || [];

    function decodeListingId(b64) {
        if (!b64) return null;
        try {
            var decoded = atob(b64);
            var parts = decoded.split(':');
            return parts.length > 1 ? parts[1] : decoded;
        } catch(e) { return null; }
    }

    var listings = results.map(function(r) {
        var id = null;
        if (r.propertyId) {
            id = r.propertyId;
        } else if (r.demandStayListing && r.demandStayListing.id) {
            id = decodeListingId(r.demandStayListing.id);
        }

        var checkin = '', checkout = '', adults = 1;
        if (r.listingParamOverrides) {
            checkin = r.listingParamOverrides.checkin || '';
            checkout = r.listingParamOverrides.checkout || '';
            adults = r.listingParamOverrides.adults || 1;
        }

        var deepLink = id
            ? 'https://www.airbnb.com/rooms/' + id +
              '?checkin=' + checkin + '&checkout=' + checkout + '&adults=' + adults
            : null;

        var price = '', originalPrice = '';
        if (r.structuredDisplayPrice) {
            var p = r.structuredDisplayPrice;
            if (p.primaryLine) price = p.primaryLine.accessibilityLabel || '';
            if (p.secondaryLine) originalPrice = p.secondaryLine.accessibilityLabel || '';
        }

        var details = '', dates = '';
        if (r.structuredContent) {
            if (r.structuredContent.primaryLine)
                details = r.structuredContent.primaryLine.map(function(x){return x.body||''}).join(' \u00b7 ');
            if (r.structuredContent.secondaryLine)
                dates = r.structuredContent.secondaryLine.map(function(x){return x.body||''}).join(' \u00b7 ');
        }

        var photoUrl = '';
        if (r.contextualPictures && r.contextualPictures[0])
            photoUrl = r.contextualPictures[0].picture || '';

        var badges = (r.badges || []).map(function(b){return b.text || b}).filter(Boolean);

        var lat = null, lng = null;
        if (r.demandStayListing && r.demandStayListing.location &&
            r.demandStayListing.location.coordinate) {
            lat = r.demandStayListing.location.coordinate.latitude;
            lng = r.demandStayListing.location.coordinate.longitude;
        }

        return {
            title: r.title || '',
            type: r.subtitle || '',
            rating: r.avgRatingLocalized || '',
            ratingDetail: r.avgRatingA11yLabel || '',
            price: price,
            originalPrice: originalPrice,
            details: details,
            dates: dates,
            badges: badges,
            photo: photoUrl,
            listingId: id,
            deepLink: deepLink,
            lat: lat,
            lng: lng,
            checkin: checkin,
            checkout: checkout,
            adults: adults
        };
    });

    var pagination = search.presentation.staysSearch.results.paginationInfo || {};

    return JSON.stringify({
        query: (niobe[0][0] || '').substring(0, 200),
        totalListings: results.length,
        listings: listings,
        hasNextPage: pagination.hasNextPage || false,
        nextPageCursor: pagination.nextPageCursor || null
    }, null, 2);
})()
