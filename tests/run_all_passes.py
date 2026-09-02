"""Master Test Runner for 5-Pass Verification of fototriagem."""

import os
import sys
import unittest

os.environ["QT_QPA_PLATFORM"] = "offscreen"

# Ensure fototriagem is on sys.path
BASE_DIR = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
if BASE_DIR not in sys.path:
    sys.path.insert(0, BASE_DIR)

from tests.test_pass1_takeout_engine import TestTakeoutEngine, TestDateExtractionEngine
from tests.test_pass2_storage_and_actions import TestStorageAndActions
from tests.test_pass3_ui_and_navigation import TestUIAndNavigation
from tests.test_pass4_async_and_multimedia import TestAsyncAndMultimedia
from tests.test_pass5_e2e_and_stress import TestE2EAndStress


def run_pass(pass_number: int, name: str, test_cases: list):
    print("=" * 70)
    print(f"RUNNING PASS {pass_number}: {name}")
    print("=" * 70)
    
    suite = unittest.TestSuite()
    for tc in test_cases:
        suite.addTests(unittest.defaultTestLoader.loadTestsFromTestCase(tc))
        
    runner = unittest.TextTestRunner(verbosity=2)
    result = runner.run(suite)
    
    if not result.wasSuccessful():
        print(f"\n[FAILED] PASS {pass_number} encountered errors/failures!")
        return False
    else:
        print(f"\n[PASSED] PASS {pass_number} completed with 0 errors/failures.")
        return True


def main():
    passes = [
        (1, "Takeout Parsing & Date Extraction Engine", [TestTakeoutEngine, TestDateExtractionEngine]),
        (2, "Storage, Deduplication, File Operations & Undo", [TestStorageAndActions]),
        (3, "UI, Menus, Keyboard Navigation & State Machine", [TestUIAndNavigation]),
        (4, "Async Concurrency, Thumbnail Cache & Multimedia", [TestAsyncAndMultimedia]),
        (5, "End-to-End Simulation & Extreme Stress Testing", [TestE2EAndStress]),
    ]
    
    all_passed = True
    for pass_num, name, tcs in passes:
        ok = run_pass(pass_num, name, tcs)
        if not ok:
            all_passed = False
            break
            
    print("\n" + "=" * 70)
    if all_passed:
        print("ALL 5 VERIFICATION PASSES COMPLETED SUCCESSFULLY WITH ZERO ERRORS!")
    else:
        print("VERIFICATION FAILED ON ONE OR MORE PASSES.")
    print("=" * 70)
    
    return 0 if all_passed else 1


if __name__ == "__main__":
    sys.exit(main())
