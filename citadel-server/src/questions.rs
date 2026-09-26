use serde::{Deserialize, Serialize};
use std::collections::HashMap;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct TestCase {
    pub input: String,
    pub expected_output: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub explanation: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct QuestionSummary {
    pub id: String,
    pub number: u32,
    pub title: String,
    pub difficulty: String,
    pub points: u32,
    pub tags: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct Question {
    pub id: String,
    pub number: u32,
    pub title: String,
    pub difficulty: String,
    pub points: u32,
    pub tags: Vec<String>,
    pub description: String,
    pub input_format: String,
    pub output_format: String,
    pub constraints: Vec<String>,
    pub sample_cases: Vec<TestCase>,
    #[serde(default)]
    pub hidden_cases: Vec<TestCase>,
    pub starter_templates: HashMap<String, String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ExamInfo {
    pub exam_id: String,
    pub title: String,
    pub institution: String,
    pub network_mode: String,
    pub duration_minutes: u32,
    pub total_questions: u32,
    pub total_points: u32,
    pub instructions: Vec<String>,
}

pub fn get_exam_info() -> ExamInfo {
    ExamInfo {
        exam_id: "citadel-campus-2026-drive".to_string(),
        title: "Campus Placement Technical Assessment 2026".to_string(),
        institution: "CITADEL Secure Offline Assessment Fleet".to_string(),
        network_mode: "Offline Campus Wi-Fi Only (Air-Gapped, Zero Internet)".to_string(),
        duration_minutes: 90,
        total_questions: 3,
        total_points: 300,
        instructions: vec![
            "This assessment runs on a zero-internet isolated local network. No external browsing is permitted.".to_string(),
            "Solve the 3 algorithmic problems (Two Sum, Three Sum, Find Largest Element) using Python, C++, or Java.".to_string(),
            "Click 'Run Code' to verify your solution against the sample test cases.".to_string(),
            "Once all sample test cases pass, the Submit button unlocks to grade your solution against the hidden test suite.".to_string(),
            "Do not attempt Alt+Tab, three-finger touchpad gestures, or opening secondary windows; all violations are logged.".to_string(),
        ],
    }
}

pub fn get_all_questions() -> Vec<Question> {
    vec![
        Question {
            id: "q1-two-sum".to_string(),
            number: 1,
            title: "Two Sum".to_string(),
            difficulty: "Easy".to_string(),
            points: 100,
            tags: vec!["Array".to_string(), "Hash Table".to_string(), "Two Pointers".to_string()],
            description: r#"Given an array of integers <code>nums</code> and an integer <code>target</code>, return <strong>indices of the two numbers</strong> such that they add up to <code>target</code>.

<p style="margin-top: 10px;">You may assume that each input would have <strong>exactly one solution</strong>, and you may not use the same element twice.</p>
<p style="margin-top: 6px;">You can return the answer in any order (indices separated by space).</p>"#.to_string(),
            input_format: "The standard input contains space-separated integers representing the array <code>nums</code> followed by the <code>target</code> value as the final integer.
Example: <code>2 7 11 15 9</code> means <code>nums = [2, 7, 11, 15]</code> and <code>target = 9</code>.".to_string(),
            output_format: "Output the two zero-based indices separated by a space on standard output (e.g. <code>0 1</code>).".to_string(),
            constraints: vec![
                "2 <= nums.length <= 10^4".to_string(),
                "-10^9 <= nums[i] <= 10^9".to_string(),
                "-10^9 <= target <= 10^9".to_string(),
                "Only one valid answer exists for each test case.".to_string(),
            ],
            sample_cases: vec![
                TestCase {
                    input: "2 7 11 15 9".to_string(),
                    expected_output: "0 1".to_string(),
                    explanation: Some("Because nums[0] + nums[1] == 2 + 7 == 9, we return indices 0 1.".to_string()),
                },
                TestCase {
                    input: "3 2 4 6".to_string(),
                    expected_output: "1 2".to_string(),
                    explanation: Some("Because nums[1] + nums[2] == 2 + 4 == 6, we return indices 1 2.".to_string()),
                },
                TestCase {
                    input: "3 3 6".to_string(),
                    expected_output: "0 1".to_string(),
                    explanation: Some("Because nums[0] + nums[1] == 3 + 3 == 6, we return indices 0 1.".to_string()),
                },
            ],
            hidden_cases: vec![
                TestCase {
                    input: "-1 -2 -3 -4 -5 -8".to_string(),
                    expected_output: "2 4".to_string(),
                    explanation: None,
                },
                TestCase {
                    input: "1 5 20 30 40 99 100".to_string(),
                    expected_output: "0 5".to_string(),
                    explanation: None,
                },
                TestCase {
                    input: "0 4 3 0 0".to_string(),
                    expected_output: "0 3".to_string(),
                    explanation: None,
                },
                TestCase {
                    input: "1 2 3 4 4 9 8".to_string(),
                    expected_output: "3 4".to_string(),
                    explanation: None,
                },
                TestCase {
                    input: "1000000000 500 1000000000 2000000000".to_string(),
                    expected_output: "0 2".to_string(),
                    explanation: None,
                },
            ],
            starter_templates: {
                let mut m = HashMap::new();
                m.insert("python".to_string(), r#"import sys

def two_sum(nums, target):
    # Write your solution here
    # Return a list containing the two indices [i, j]
    pass

if __name__ == '__main__':
    tokens = sys.stdin.read().split()
    if tokens:
        target = int(tokens[-1])
        nums = [int(x) for x in tokens[:-1]]
        ans = two_sum(nums, target)
        if ans and len(ans) == 2:
            print(f"{min(ans[0], ans[1])} {max(ans[0], ans[1])}")
"#.to_string());

                m.insert("cpp".to_string(), r#"#include <iostream>
#include <vector>
#include <unordered_map>
#include <algorithm>

using namespace std;

vector<int> twoSum(vector<int>& nums, int target) {
    // Write your solution here
    // Return vector containing the two indices
    return {};
}

int main() {
    ios_base::sync_with_stdio(false);
    cin.tie(NULL);

    vector<int> nums;
    int val;
    while (cin >> val) {
        nums.push_back(val);
    }
    if (nums.size() >= 2) {
        int target = nums.back();
        nums.pop_back();
        vector<int> ans = twoSum(nums, target);
        if (ans.size() == 2) {
            cout << min(ans[0], ans[1]) << " " << max(ans[0], ans[1]) << "
";
        }
    }
    return 0;
}
"#.to_string());

                m.insert("java".to_string(), r#"import java.util.*;

public class Solution {
    public static int[] twoSum(int[] nums, int target) {
        // Write your solution here
        // Return int array with two indices
        return new int[]{};
    }

    public static void main(String[] args) {
        Scanner sc = new Scanner(System.in);
        List<Integer> list = new ArrayList<>();
        while (sc.hasNextInt()) {
            list.add(sc.nextInt());
        }
        if (list.size() >= 2) {
            int target = list.remove(list.size() - 1);
            int[] nums = new int[list.size()];
            for (int i = 0; i < list.size(); i++) {
                nums[i] = list.get(i);
            }
            int[] ans = twoSum(nums, target);
            if (ans != null && ans.length == 2) {
                System.out.println(Math.min(ans[0], ans[1]) + " " + Math.max(ans[0], ans[1]));
            }
        }
    }
}
"#.to_string());
                m
            },
        },
        Question {
            id: "q2-three-sum".to_string(),
            number: 2,
            title: "Three Sum".to_string(),
            difficulty: "Medium".to_string(),
            points: 150,
            tags: vec!["Array".to_string(), "Two Pointers".to_string(), "Sorting".to_string()],
            description: r#"Given an integer array <code>nums</code>, return all the triplets <code>[nums[i], nums[j], nums[k]]</code> such that <code>i != j</code>, <code>i != k</code>, and <code>j != k</code>, and <code>nums[i] + nums[j] + nums[k] == 0</code>.

<p style="margin-top: 10px;">Notice that the solution set must not contain duplicate triplets. Print each triplet on a separate line with its 3 numbers in ascending order separated by space.</p>"#.to_string(),
            input_format: "Standard input contains space-separated integers representing the array <code>nums</code>.".to_string(),
            output_format: "Output each unique triplet on a new line with space-separated numbers sorted in ascending order. If no triplet exists, output nothing.".to_string(),
            constraints: vec![
                "3 <= nums.length <= 3000".to_string(),
                "-10^5 <= nums[i] <= 10^5".to_string(),
            ],
            sample_cases: vec![
                TestCase {
                    input: "-1 0 1 2 -1 -4".to_string(),
                    expected_output: "-1 -1 2
-1 0 1".to_string(),
                    explanation: Some("Distinct triplets that sum to 0 are [-1, -1, 2] and [-1, 0, 1].".to_string()),
                },
                TestCase {
                    input: "0 1 1".to_string(),
                    expected_output: "".to_string(),
                    explanation: Some("No possible triplet sums to 0.".to_string()),
                },
                TestCase {
                    input: "0 0 0".to_string(),
                    expected_output: "0 0 0".to_string(),
                    explanation: Some("The only possible triplet sums to 0.".to_string()),
                },
            ],
            hidden_cases: vec![
                TestCase {
                    input: "-2 0 1 1 2".to_string(),
                    expected_output: "-2 0 2
-2 1 1".to_string(),
                    explanation: None,
                },
                TestCase {
                    input: "-4 -2 -2 -2 0 1 2 2 2 3 3 4 4 6 6".to_string(),
                    expected_output: "-4 -2 6
-4 0 4
-4 1 3
-4 2 2
-2 -2 4
-2 0 2".to_string(),
                    explanation: None,
                },
                TestCase {
                    input: "1 2 -2 -1".to_string(),
                    expected_output: "".to_string(),
                    explanation: None,
                },
                TestCase {
                    input: "-1 -1 0 1 1".to_string(),
                    expected_output: "-1 0 1".to_string(),
                    explanation: None,
                },
                TestCase {
                    input: "-5 2 3 -4 1 3 0 0 0".to_string(),
                    expected_output: "-5 2 3
-4 1 3
0 0 0".to_string(),
                    explanation: None,
                },
            ],
            starter_templates: {
                let mut m = HashMap::new();
                m.insert("python".to_string(), r#"import sys

def three_sum(nums):
    # Write your solution here
    # Return a list of triplets, e.g. [[-1, -1, 2], [-1, 0, 1]]
    return []

if __name__ == '__main__':
    tokens = sys.stdin.read().split()
    if tokens:
        nums = [int(x) for x in tokens]
        triplets = three_sum(nums)
        for t in triplets:
            t.sort()
        triplets.sort()
        for t in triplets:
            print(f"{t[0]} {t[1]} {t[2]}")
"#.to_string());

                m.insert("cpp".to_string(), r#"#include <iostream>
#include <vector>
#include <algorithm>

using namespace std;

vector<vector<int>> threeSum(vector<int>& nums) {
    // Write your solution here
    return {};
}

int main() {
    ios_base::sync_with_stdio(false);
    cin.tie(NULL);

    vector<int> nums;
    int val;
    while (cin >> val) {
        nums.push_back(val);
    }
    vector<vector<int>> ans = threeSum(nums);
    for (auto& t : ans) {
        sort(t.begin(), t.end());
    }
    sort(ans.begin(), ans.end());
    for (const auto& t : ans) {
        if (t.size() == 3) {
            cout << t[0] << " " << t[1] << " " << t[2] << "
";
        }
    }
    return 0;
}
"#.to_string());

                m.insert("java".to_string(), r#"import java.util.*;

public class Solution {
    public static List<List<Integer>> threeSum(int[] nums) {
        // Write your solution here
        return new ArrayList<>();
    }

    public static void main(String[] args) {
        Scanner sc = new Scanner(System.in);
        List<Integer> list = new ArrayList<>();
        while (sc.hasNextInt()) {
            list.add(sc.nextInt());
        }
        int[] nums = new int[list.size()];
        for (int i = 0; i < list.size(); i++) {
            nums[i] = list.get(i);
        }
        List<List<Integer>> ans = threeSum(nums);
        for (List<Integer> t : ans) {
            Collections.sort(t);
        }
        ans.sort((a, b) -> {
            for (int i = 0; i < 3; i++) {
                int cmp = Integer.compare(a.get(i), b.get(i));
                if (cmp != 0) return cmp;
            }
            return 0;
        });
        for (List<Integer> t : ans) {
            if (t.size() == 3) {
                System.out.println(t.get(0) + " " + t.get(1) + " " + t.get(2));
            }
        }
    }
}
"#.to_string());
                m
            },
        },
        Question {
            id: "q3-find-largest-element".to_string(),
            number: 3,
            title: "Find Largest Element".to_string(),
            difficulty: "Easy".to_string(),
            points: 50,
            tags: vec!["Array".to_string(), "Basics".to_string()],
            description: r#"Given an array of integers <code>nums</code>, find and return the <strong>maximum value</strong> present in the array.

<p style="margin-top: 10px;">The array contains at least one integer.</p>"#.to_string(),
            input_format: "Standard input contains space-separated integers representing the array <code>nums</code>.".to_string(),
            output_format: "Output the single maximum integer value on standard output.".to_string(),
            constraints: vec![
                "1 <= nums.length <= 10^5".to_string(),
                "-10^9 <= nums[i] <= 10^9".to_string(),
            ],
            sample_cases: vec![
                TestCase {
                    input: "3 1 4 1 5 9 2 6".to_string(),
                    expected_output: "9".to_string(),
                    explanation: Some("The maximum number in the array is 9.".to_string()),
                },
                TestCase {
                    input: "-5 -2 -8 -1".to_string(),
                    expected_output: "-1".to_string(),
                    explanation: Some("Among negative numbers, -1 is the largest.".to_string()),
                },
                TestCase {
                    input: "42".to_string(),
                    expected_output: "42".to_string(),
                    explanation: Some("Single element array has max equal to itself.".to_string()),
                },
            ],
            hidden_cases: vec![
                TestCase {
                    input: "100".to_string(),
                    expected_output: "100".to_string(),
                    explanation: None,
                },
                TestCase {
                    input: "7 7 7 7 7".to_string(),
                    expected_output: "7".to_string(),
                    explanation: None,
                },
                TestCase {
                    input: "-1000000000 0 1000000000".to_string(),
                    expected_output: "1000000000".to_string(),
                    explanation: None,
                },
                TestCase {
                    input: "1 2 3 4 5 6 7 8 9 10".to_string(),
                    expected_output: "10".to_string(),
                    explanation: None,
                },
                TestCase {
                    input: "10 9 8 7 6 5 4 3 2 1".to_string(),
                    expected_output: "10".to_string(),
                    explanation: None,
                },
            ],
            starter_templates: {
                let mut m = HashMap::new();
                m.insert("python".to_string(), r#"import sys

def find_largest(nums):
    # Write your solution here
    # Return the maximum element in nums
    return nums[0] if nums else 0

if __name__ == '__main__':
    tokens = sys.stdin.read().split()
    if tokens:
        nums = [int(x) for x in tokens]
        print(find_largest(nums))
"#.to_string());

                m.insert("cpp".to_string(), r#"#include <iostream>
#include <vector>
#include <algorithm>

using namespace std;

int findLargest(const vector<int>& nums) {
    // Write your solution here
    return 0;
}

int main() {
    ios_base::sync_with_stdio(false);
    cin.tie(NULL);

    vector<int> nums;
    int val;
    while (cin >> val) {
        nums.push_back(val);
    }
    if (!nums.empty()) {
        cout << findLargest(nums) << "
";
    }
    return 0;
}
"#.to_string());

                m.insert("java".to_string(), r#"import java.util.*;

public class Solution {
    public static int findLargest(int[] nums) {
        // Write your solution here
        return 0;
    }

    public static void main(String[] args) {
        Scanner sc = new Scanner(System.in);
        List<Integer> list = new ArrayList<>();
        while (sc.hasNextInt()) {
            list.add(sc.nextInt());
        }
        int[] nums = new int[list.size()];
        for (int i = 0; i < list.size(); i++) {
            nums[i] = list.get(i);
        }
        if (nums.length > 0) {
            System.out.println(findLargest(nums));
        }
    }
}
"#.to_string());
                m
            },
        },
    ]
}

pub fn get_question_summaries(questions: &[Question]) -> Vec<QuestionSummary> {
    questions
        .iter()
        .map(|q| QuestionSummary {
            id: q.id.clone(),
            number: q.number,
            title: q.title.clone(),
            difficulty: q.difficulty.clone(),
            points: q.points,
            tags: q.tags.clone(),
        })
        .collect()
}

pub fn sanitize_for_candidate(mut q: Question) -> Question {
    q.hidden_cases.clear();
    q
}
