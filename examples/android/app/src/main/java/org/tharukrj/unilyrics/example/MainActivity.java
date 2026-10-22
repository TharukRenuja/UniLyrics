package org.tharukrj.unilyrics.example;

import android.app.Activity;
import android.os.Bundle;
import android.widget.TextView;

public final class MainActivity extends Activity {
    @SuppressWarnings("deprecation")
    @Override public void onCreate(Bundle state) {
        super.onCreate(state);
        // Target SDK 37 lays content behind system bars and the legacy action bar.
        if (getActionBar() != null) getActionBar().hide();
        var text = new TextView(this);
        int padding = Math.round(24 * getResources().getDisplayMetrics().density);
        text.setTextSize(20);
        text.setOnApplyWindowInsetsListener((view, insets) -> {
            view.setPadding(padding + insets.getSystemWindowInsetLeft(),
                padding + insets.getSystemWindowInsetTop(),
                padding + insets.getSystemWindowInsetRight(),
                padding + insets.getSystemWindowInsetBottom());
            return insets;
        });
        setContentView(text);
        text.requestApplyInsets();
        try {
            text.setText("Java: " + JavaExample.run() + "\nKotlin: " + KotlinExample.run());
        } catch (RuntimeException | LinkageError error) {
            text.setText("SDK initialization failed: " + error);
        }
    }
}
