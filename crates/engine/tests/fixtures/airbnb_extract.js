// Airbnb listing extraction script.
//
// Extracts listing data from Airbnb-style search results HTML.
// Uses querySelectorAll to find listing cards, then extracts
// title, price, and rating from each.

(function() {
    var cards = document.querySelectorAll('[data-testid="listing-card"]');
    if (!cards || cards.length === 0) {
        // Fallback: try class-based selection
        cards = document.querySelectorAll('.listing');
    }

    var listings = [];
    for (var i = 0; i < cards.length; i++) {
        var card = cards[i];
        var title = '';
        var price = '';
        var rating = '';

        // Extract title
        var titleEl = card.querySelector('.listing-title');
        if (titleEl) title = titleEl.textContent.trim();

        // Extract price
        var priceEl = card.querySelector('.listing-price');
        if (priceEl) price = priceEl.textContent.trim();

        // Extract rating
        var ratingEl = card.querySelector('.listing-rating');
        if (ratingEl) rating = ratingEl.textContent.trim();

        listings.push({
            title: title,
            price: price,
            rating: rating
        });
    }

    return JSON.stringify({ count: listings.length, listings: listings });
})()
