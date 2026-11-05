part of 'generated.dart';

class ListMyProfilesVariablesBuilder {
  
  final FirebaseDataConnect _dataConnect;
  ListMyProfilesVariablesBuilder(this._dataConnect, );
  Deserializer<ListMyProfilesData> dataDeserializer = (dynamic json)  => ListMyProfilesData.fromJson(jsonDecode(json));
  
  Future<QueryResult<ListMyProfilesData, void>> execute({QueryFetchPolicy fetchPolicy = QueryFetchPolicy.preferCache}) {
    return ref().execute(fetchPolicy: fetchPolicy);
  }

  QueryRef<ListMyProfilesData, void> ref() {
    
    return _dataConnect.query("ListMyProfiles", dataDeserializer, emptySerializer, null);
  }
}

@immutable
class ListMyProfilesProfiles {
  final String name;
  final bool? isDefault;
  ListMyProfilesProfiles.fromJson(dynamic json):
  
  name = nativeFromJson<String>(json['name']),
  isDefault = json['isDefault'] == null ? null : nativeFromJson<bool>(json['isDefault']);
  @override
  bool operator ==(Object other) {
    if(identical(this, other)) {
      return true;
    }
    if(other.runtimeType != runtimeType) {
      return false;
    }

    final ListMyProfilesProfiles otherTyped = other as ListMyProfilesProfiles;
    return name == otherTyped.name && 
    isDefault == otherTyped.isDefault;
    
  }
  @override
  int get hashCode => Object.hashAll([name.hashCode, isDefault.hashCode]);
  

  Map<String, dynamic> toJson() {
    Map<String, dynamic> json = {};
    json['name'] = nativeToJson<String>(name);
    if (isDefault != null) {
      json['isDefault'] = nativeToJson<bool?>(isDefault);
    }
    return json;
  }

  ListMyProfilesProfiles({
    required this.name,
    this.isDefault,
  });
}

@immutable
class ListMyProfilesData {
  final List<ListMyProfilesProfiles> profiles;
  ListMyProfilesData.fromJson(dynamic json):
  
  profiles = (json['profiles'] as List<dynamic>)
        .map((e) => ListMyProfilesProfiles.fromJson(e))
        .toList();
  @override
  bool operator ==(Object other) {
    if(identical(this, other)) {
      return true;
    }
    if(other.runtimeType != runtimeType) {
      return false;
    }

    final ListMyProfilesData otherTyped = other as ListMyProfilesData;
    return profiles == otherTyped.profiles;
    
  }
  @override
  int get hashCode => profiles.hashCode;
  

  Map<String, dynamic> toJson() {
    Map<String, dynamic> json = {};
    json['profiles'] = profiles.map((e) => e.toJson()).toList();
    return json;
  }

  ListMyProfilesData({
    required this.profiles,
  });
}

